//! Animation-controller integration for the orchestrator.
//!
//! The animation controller owns a `vizij-animation-core` engine instance and translates
//! blackboard paths into player commands and instance updates on each orchestrator step.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value as JsonValue;

use vizij_animation_core::{
    config::Config,
    ids::{InstId, PlayerId},
    inputs::{Inputs, InstanceUpdate, PlayerCommand},
    stored_animation::parse_stored_animation_json,
    Engine, InstanceCfg, LoopMode,
};
use vizij_api_core::{TypedPath, Value as ApiValue, WriteBatch};

use crate::blackboard::Blackboard;

/// Lightweight config for registering an animation controller with the orchestrator.
#[derive(Debug, Clone)]
pub struct AnimationControllerConfig {
    /// Controller id used for registration and diagnostics.
    pub id: String,
    /// Optional setup object interpreted as `{ animation?, player?, instance? }`.
    ///
    /// Unknown fields are ignored by serde. When `animation` is present it must match the stored
    /// animation JSON format accepted by `parse_stored_animation_json`.
    pub setup: JsonValue,
}

#[derive(Debug)]
pub struct AnimationController {
    /// Controller id used by the orchestrator registry.
    pub id: String,
    /// Owned animation engine instance.
    pub engine: Engine,
}

enum AnimationPathKind<'a> {
    PlayerCommand {
        controller: Option<String>,
        player: PlayerId,
        action: &'a str,
    },
    InstanceField {
        controller: Option<String>,
        player: PlayerId,
        inst: InstId,
        field: String,
    },
}

impl AnimationController {
    /// Construct a controller, returning an error when the setup payload is invalid.
    pub fn try_new(cfg: AnimationControllerConfig) -> Result<Self> {
        // Create engine with a default config, then apply any optional setup payload.
        let mut controller = Self {
            id: cfg.id,
            engine: Engine::new(Config::default()),
        };
        controller.configure_from_setup(&cfg.setup)?;
        Ok(controller)
    }

    /// Construct a controller and panic if the setup payload is invalid.
    pub fn new(cfg: AnimationControllerConfig) -> Self {
        Self::try_new(cfg).expect("AnimationController setup is invalid")
    }

    fn configure_from_setup(&mut self, setup: &JsonValue) -> Result<()> {
        if setup.is_null() {
            return Ok(());
        }

        let spec: AnimationSetup =
            serde_json::from_value(setup.clone()).context("animation setup must be an object")?;

        let mut anim_id = None;
        if let Some(anim_value) = spec.animation {
            let json = serde_json::to_string(&anim_value)?;
            let data = parse_stored_animation_json(&json)
                .map_err(|e| anyhow!("stored animation parse error: {e}"))?;
            let id = self.engine.load_animation(data);
            anim_id = Some(id);
        }

        if let Some(anim) = anim_id {
            let player_cfg = spec.player.unwrap_or_default();
            let player_name = player_cfg
                .name
                .clone()
                .unwrap_or_else(|| "demo-player".to_string());
            let player_id = self.engine.create_player(&player_name);

            self.apply_player_overrides(player_id, &player_cfg);

            let instance_cfg = spec.instance.map(InstanceCfg::from).unwrap_or_default();
            self.engine.add_instance(player_id, anim, instance_cfg);
        }

        Ok(())
    }

    fn apply_player_overrides(&mut self, player_id: PlayerId, cfg: &PlayerSetup) {
        if cfg.loop_mode.is_none() && cfg.speed.is_none() {
            return;
        }

        // Commands are the public way to mutate player state; enqueue them so the engine applies them next update.
        let mut inputs = Inputs::default();

        if let Some(mode) = cfg.loop_mode.as_deref() {
            let loop_mode = match mode {
                "once" => LoopMode::Once,
                "loop" => LoopMode::Loop,
                "pingpong" => LoopMode::PingPong,
                _ => LoopMode::Loop,
            };
            inputs.player_cmds.push(PlayerCommand::SetLoopMode {
                player: player_id,
                mode: loop_mode,
            });
        }

        if let Some(speed) = cfg.speed {
            inputs.player_cmds.push(PlayerCommand::SetSpeed {
                player: player_id,
                speed,
            });
        }

        if !inputs.player_cmds.is_empty() {
            self.engine.update_values(0.0, inputs);
        }
    }

    /// Minimal helper to parse u32 from a path segment.
    fn parse_u32_segment(s: &str) -> Option<u32> {
        s.parse::<u32>().ok()
    }

    fn classify_path<'a>(tp: &'a TypedPath) -> Option<AnimationPathKind<'a>> {
        if tp.namespace_segment(0)? != "anim" {
            return None;
        }

        if tp.namespace_segment(1)? == "player" {
            return Self::classify_player_path(tp, None, 1);
        }

        if tp.namespace_segment(1)? != "controller" {
            return None;
        }

        let player_idx = (2..tp.namespaces.len()).find(|idx| {
            tp.namespace_segment(*idx) == Some("player")
                && tp
                    .namespace_segment(*idx + 1)
                    .and_then(Self::parse_u32_segment)
                    .is_some()
                && matches!(tp.namespace_segment(*idx + 2), Some("cmd" | "instance"))
        })?;
        if player_idx <= 2 {
            return None;
        }
        let controller = tp.namespaces[2..player_idx].join("/");
        Self::classify_player_path(tp, Some(controller), player_idx)
    }

    fn classify_player_path<'a>(
        tp: &'a TypedPath,
        controller: Option<String>,
        player_idx: usize,
    ) -> Option<AnimationPathKind<'a>> {
        let player_id = Self::parse_u32_segment(tp.namespace_segment(player_idx + 1)?)?;
        match tp.namespace_segment(player_idx + 2)? {
            "cmd" => Some(AnimationPathKind::PlayerCommand {
                controller,
                player: PlayerId(player_id),
                action: tp.target_name(),
            }),
            "instance" => {
                let inst_id = Self::parse_u32_segment(tp.namespace_segment(player_idx + 3)?)?;
                Some(AnimationPathKind::InstanceField {
                    controller,
                    player: PlayerId(player_id),
                    inst: InstId(inst_id),
                    field: Self::compose_field_name(tp),
                })
            }
            _ => None,
        }
    }

    fn compose_field_name(tp: &TypedPath) -> String {
        if tp.fields.is_empty() {
            tp.target.clone()
        } else {
            let mut name = tp.target.clone();
            name.push('.');
            name.push_str(&tp.fields.join("."));
            name
        }
    }

    fn path_matches_controller(controller: Option<&str>, filter: Option<&str>) -> bool {
        match (controller, filter) {
            (None, _) => true,
            (Some(actual), Some(expected)) => actual == expected,
            (Some(_), None) => false,
        }
    }

    fn parse_loop_mode_value(value: &ApiValue) -> Option<LoopMode> {
        let text = match value {
            ApiValue::Text(text) => text.as_str(),
            _ => return None,
        };
        match text.trim().to_ascii_lowercase().as_str() {
            "once" => Some(LoopMode::Once),
            "loop" => Some(LoopMode::Loop),
            "pingpong" | "ping_pong" | "ping-pong" => Some(LoopMode::PingPong),
            _ => None,
        }
    }

    /// Map Blackboard entries into Engine Inputs using a small convention:
    ///
    /// - Player-level commands:
    ///   TypedPath: "anim/player/<player_id>/cmd/<action>"
    ///   TypedPath: "anim/controller/<controller_id>/player/<player_id>/cmd/<action>"
    ///   where <action> is one of:
    ///     - "play", "pause", "stop"
    ///     - "set_speed" (value must be Float)
    ///     - "seek" (value must be Float)
    ///     - "set_loop" (value must be Text: "once" | "loop" | "pingpong")
    ///
    /// - Instance updates:
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/weight"
    ///   TypedPath: "anim/controller/<controller_id>/player/<player_id>/instance/<inst_id>/weight"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/time_scale"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/start_offset"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/enabled"
    ///
    /// These conventions are intentionally conservative and documented for now.
    /// Build animation-engine inputs from the orchestrator blackboard using the shared
    /// animation command path convention.
    pub fn inputs_from_blackboard(bb: &Blackboard) -> Inputs {
        Self::inputs_from_blackboard_with_filter(bb, None)
    }

    /// Build animation-engine inputs for a specific controller id.
    ///
    /// Legacy `anim/player/...` paths still apply as broadcast compatibility commands. Scoped
    /// `anim/controller/<controller_id>/...` paths apply only when the id matches this controller.
    pub fn inputs_from_blackboard_for_controller(bb: &Blackboard, controller_id: &str) -> Inputs {
        Self::inputs_from_blackboard_with_filter(bb, Some(controller_id))
    }

    fn inputs_from_blackboard_with_filter(
        bb: &Blackboard,
        controller_filter: Option<&str>,
    ) -> Inputs {
        let mut inputs = Inputs::default();

        for (tp, entry) in bb.iter() {
            match Self::classify_path(tp) {
                Some(AnimationPathKind::PlayerCommand {
                    controller,
                    player,
                    action,
                }) if Self::path_matches_controller(controller.as_deref(), controller_filter) => {
                    match action {
                        "play" => inputs.player_cmds.push(PlayerCommand::Play { player }),
                        "pause" => inputs.player_cmds.push(PlayerCommand::Pause { player }),
                        "stop" => inputs.player_cmds.push(PlayerCommand::Stop { player }),
                        "set_speed" => {
                            if let ApiValue::Float(f) = &entry.value {
                                inputs
                                    .player_cmds
                                    .push(PlayerCommand::SetSpeed { player, speed: *f });
                            }
                        }
                        "seek" => {
                            if let ApiValue::Float(f) = &entry.value {
                                inputs
                                    .player_cmds
                                    .push(PlayerCommand::Seek { player, time: *f });
                            }
                        }
                        "set_loop" | "set_loop_mode" => {
                            if let Some(mode) = Self::parse_loop_mode_value(&entry.value) {
                                inputs
                                    .player_cmds
                                    .push(PlayerCommand::SetLoopMode { player, mode });
                            }
                        }
                        _ => {
                            // Unknown action - skip for now
                        }
                    }
                }
                Some(AnimationPathKind::InstanceField {
                    controller,
                    player,
                    inst,
                    field,
                }) if Self::path_matches_controller(controller.as_deref(), controller_filter) => {
                    let mut upd = InstanceUpdate {
                        player,
                        inst,
                        weight: None,
                        time_scale: None,
                        start_offset: None,
                        enabled: None,
                    };
                    match field.as_str() {
                        "weight" => {
                            if let ApiValue::Float(f) = &entry.value {
                                upd.weight = Some(*f);
                                inputs.instance_updates.push(upd);
                            }
                        }
                        "time_scale" => {
                            if let ApiValue::Float(f) = &entry.value {
                                upd.time_scale = Some(*f);
                                inputs.instance_updates.push(upd);
                            }
                        }
                        "start_offset" => {
                            if let ApiValue::Float(f) = &entry.value {
                                upd.start_offset = Some(*f);
                                inputs.instance_updates.push(upd);
                            }
                        }
                        "enabled" => {
                            if let ApiValue::Bool(b) = &entry.value {
                                upd.enabled = Some(*b);
                                inputs.instance_updates.push(upd);
                            }
                        }
                        _ => {}
                    }
                }
                Some(_) => {}
                None => {}
            }
        }

        inputs
    }

    /// Update the animation controller for `dt` seconds, reading relevant inputs from the
    /// blackboard and returning a WriteBatch plus a list of high-level event values.
    ///
    /// Behavior:
    ///  - Build `Inputs` from the Blackboard using a small path convention
    ///  - Call `engine.update_values(dt, inputs)` to advance the engine and collect Outputs
    ///  - Translate `Outputs.changes` into a WriteBatch using the shared helper on
    ///    `vizij_animation_core::Outputs`, which handles the TypedPath parsing.
    ///  - Serialize engine events into `serde_json::Value` and return them alongside the batch.
    ///
    /// Unsupported command paths or value types are ignored rather than treated as hard errors.
    pub fn update(&mut self, dt: f32, bb: &mut Blackboard) -> Result<(WriteBatch, Vec<JsonValue>)> {
        // Build Inputs from Blackboard
        let inputs = Self::inputs_from_blackboard_for_controller(bb, &self.id);

        // Step engine and get outputs reference
        let outputs = self.engine.update_values(dt, inputs);

        // Build WriteBatch from engine outputs using the shared helper
        let batch = outputs.to_writebatch();

        // Serialize events to JSON for diagnostics/consumers
        let mut events: Vec<JsonValue> = Vec::new();
        for ev in outputs.events.iter() {
            if let Ok(v) = serde_json::to_value(ev) {
                events.push(v);
            }
        }

        Ok((batch, events))
    }
}

#[derive(Debug, Default, Deserialize)]
struct AnimationSetup {
    #[serde(default)]
    animation: Option<JsonValue>,
    #[serde(default)]
    player: Option<PlayerSetup>,
    #[serde(default)]
    instance: Option<InstanceSetup>,
}

#[derive(Debug, Default, Deserialize)]
struct PlayerSetup {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "loopMode")]
    loop_mode: Option<String>,
    #[serde(default)]
    speed: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
struct InstanceSetup {
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default, alias = "timeScale", alias = "timescale")]
    time_scale: Option<f32>,
    #[serde(default, alias = "startOffset")]
    start_offset: Option<f32>,
    /// Studio instance offset in milliseconds. `startOffset`/`start_offset` remain seconds.
    #[serde(default)]
    offset: Option<f32>,
    #[serde(default, alias = "active")]
    enabled: Option<bool>,
}

impl From<InstanceSetup> for InstanceCfg {
    fn from(setup: InstanceSetup) -> Self {
        let mut config = InstanceCfg::default();
        if let Some(weight) = setup.weight {
            config.weight = weight;
        }
        if let Some(time_scale) = setup.time_scale {
            config.time_scale = time_scale;
        }
        if let Some(start_offset) = setup.start_offset {
            config.start_offset = start_offset;
        } else if let Some(offset_ms) = setup.offset {
            config.start_offset = offset_ms / 1000.0;
        }
        if let Some(enabled) = setup.enabled {
            config.enabled = enabled;
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::Blackboard;

    #[test]
    fn map_blackboard_play_command_and_instance_update() {
        let mut bb = Blackboard::new();
        // Player command: play
        bb.set(
            "anim/player/0/cmd/play".to_string(),
            serde_json::json!({"type":"float","data":0.0}),
            None,
            0,
            "test".into(),
        )
        .expect("set player cmd");

        // Instance weight update
        bb.set(
            "anim/player/0/instance/1/weight".to_string(),
            serde_json::json!({"type":"float","data":0.75}),
            None,
            0,
            "test".into(),
        )
        .expect("set instance weight");

        let inputs = AnimationController::inputs_from_blackboard(&bb);
        // Expect one Play command and one InstanceUpdate with weight 0.75
        assert!(inputs
            .player_cmds
            .iter()
            .any(|c| matches!(c, PlayerCommand::Play { player } if player.0 == 0)));
        assert!(inputs
            .instance_updates
            .iter()
            .any(|u| u.player.0 == 0 && u.inst.0 == 1 && u.weight == Some(0.75)));
    }

    #[test]
    fn scoped_blackboard_commands_apply_only_to_matching_controller() {
        let mut bb = Blackboard::new();
        bb.set(
            "anim/controller/default/animation/blink/player/0/cmd/play".to_string(),
            serde_json::json!({"type":"bool","data":true}),
            None,
            0,
            "test".into(),
        )
        .expect("set scoped play");
        bb.set(
            "anim/controller/default/animation/smile/player/0/cmd/set_speed".to_string(),
            serde_json::json!({"type":"float","data":2.0}),
            None,
            0,
            "test".into(),
        )
        .expect("set scoped speed");
        bb.set(
            "anim/player/0/cmd/seek".to_string(),
            serde_json::json!({"type":"float","data":0.25}),
            None,
            0,
            "test".into(),
        )
        .expect("set broadcast seek");

        let blink = AnimationController::inputs_from_blackboard_for_controller(
            &bb,
            "default/animation/blink",
        );
        assert!(blink
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::Play { player } if player.0 == 0)));
        assert!(blink
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::Seek { player, time } if player.0 == 0 && (*time - 0.25).abs() < 0.0001)));
        assert!(!blink
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::SetSpeed { .. })));

        let smile = AnimationController::inputs_from_blackboard_for_controller(
            &bb,
            "default/animation/smile",
        );
        assert!(smile
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::SetSpeed { player, speed } if player.0 == 0 && (*speed - 2.0).abs() < 0.0001)));
        assert!(smile
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::Seek { player, time } if player.0 == 0 && (*time - 0.25).abs() < 0.0001)));
        assert!(!smile
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::Play { .. })));
    }

    #[test]
    fn scoped_blackboard_commands_allow_player_segment_inside_controller_id() {
        let mut bb = Blackboard::new();
        bb.set(
            "anim/controller/demo-player/animation/blink/player/0/cmd/seek".to_string(),
            serde_json::json!({"type":"float","data":0.4}),
            None,
            0,
            "test".into(),
        )
        .expect("set scoped seek");

        let inputs = AnimationController::inputs_from_blackboard_for_controller(
            &bb,
            "demo-player/animation/blink",
        );
        assert!(inputs
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::Seek { player, time } if player.0 == 0 && (*time - 0.4).abs() < 0.0001)));
    }

    #[test]
    fn scoped_blackboard_loop_and_instance_updates_are_controller_specific() {
        let mut bb = Blackboard::new();
        bb.set(
            "anim/controller/default/animation/blink/player/0/cmd/set_loop".to_string(),
            serde_json::json!({"type":"text","data":"pingpong"}),
            None,
            0,
            "test".into(),
        )
        .expect("set scoped loop");
        bb.set(
            "anim/controller/default/animation/blink/player/0/instance/0/weight".to_string(),
            serde_json::json!({"type":"float","data":0.5}),
            None,
            0,
            "test".into(),
        )
        .expect("set scoped instance weight");

        let blink = AnimationController::inputs_from_blackboard_for_controller(
            &bb,
            "default/animation/blink",
        );
        assert!(blink
            .player_cmds
            .iter()
            .any(|cmd| matches!(cmd, PlayerCommand::SetLoopMode { player, mode } if player.0 == 0 && *mode == LoopMode::PingPong)));
        assert!(blink
            .instance_updates
            .iter()
            .any(|update| update.player.0 == 0
                && update.inst.0 == 0
                && update.weight == Some(0.5)));

        let smile = AnimationController::inputs_from_blackboard_for_controller(
            &bb,
            "default/animation/smile",
        );
        assert!(smile.player_cmds.is_empty());
        assert!(smile.instance_updates.is_empty());
    }
}
