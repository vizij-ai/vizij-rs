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
use vizij_api_core::{TypedPath, Value as ApiValue, WriteBatch, WriteOp};

use crate::blackboard::Blackboard;

/// Lightweight config for registering an animation controller with the orchestrator.
#[derive(Debug, Clone)]
pub struct AnimationControllerConfig {
    pub id: String,
    /// Arbitrary setup blob for future wiring (e.g., animation JSON, prebind config).
    pub setup: JsonValue,
}

#[derive(Debug)]
pub struct AnimationController {
    pub id: String,
    pub engine: Engine,
}

impl AnimationController {
    pub fn try_new(cfg: AnimationControllerConfig) -> Result<Self> {
        // Create engine with a default config, then apply any optional setup payload.
        let mut controller = Self {
            id: cfg.id,
            engine: Engine::new(Config::default()),
        };
        controller.configure_from_setup(&cfg.setup)?;
        Ok(controller)
    }

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

            let mut instance_cfg = InstanceCfg::default();
            if let Some(inst) = spec.instance {
                if let Some(weight) = inst.weight {
                    instance_cfg.weight = weight;
                }
                if let Some(time_scale) = inst.time_scale {
                    instance_cfg.time_scale = time_scale;
                }
                if let Some(start_offset) = inst.start_offset {
                    instance_cfg.start_offset = start_offset;
                }
                if let Some(enabled) = inst.enabled {
                    instance_cfg.enabled = enabled;
                }
            }
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

    /// Map Blackboard entries into Engine Inputs using a small convention:
    ///
    /// - Player-level commands:
    ///   TypedPath: "anim/player/<player_id>/cmd/<action>"
    ///   where <action> is one of:
    ///     - "play", "pause", "stop"
    ///     - "set_speed" (value must be Float)
    ///     - "seek" (value must be Float)
    ///     - "set_loop" (value: "once" | "loop" | "pingpong") -- not implemented here
    ///
    /// - Instance updates:
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/weight"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/time_scale"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/start_offset"
    ///   TypedPath: "anim/player/<player_id>/instance/<inst_id>/enabled"
    ///
    /// These conventions are intentionally conservative and documented for now.
    fn map_blackboard_to_inputs(bb: &Blackboard) -> Inputs {
        let mut inputs = Inputs::default();

        for (tp, entry) in bb.iter() {
            // Use the string form of the TypedPath for simple convention parsing.
            let s = tp.to_string();
            // split and ignore empty segments
            let segs: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
            if segs.len() < 4 {
                continue;
            }

            // Expect prefix "anim" then "player" then <player_id>
            if segs[0] != "anim" || segs[1] != "player" {
                continue;
            }

            let player_id = match Self::parse_u32_segment(segs[2]) {
                Some(n) => PlayerId(n),
                None => continue,
            };

            // Player command path: anim/player/<pid>/cmd/<action>
            if segs.len() >= 5 && segs[3] == "cmd" {
                let action = segs[4];
                match action {
                    "play" => inputs
                        .player_cmds
                        .push(PlayerCommand::Play { player: player_id }),
                    "pause" => inputs
                        .player_cmds
                        .push(PlayerCommand::Pause { player: player_id }),
                    "stop" => inputs
                        .player_cmds
                        .push(PlayerCommand::Stop { player: player_id }),
                    "set_speed" => {
                        if let ApiValue::Float(f) = &entry.value {
                            inputs.player_cmds.push(PlayerCommand::SetSpeed {
                                player: player_id,
                                speed: *f,
                            });
                        }
                    }
                    "seek" => {
                        if let ApiValue::Float(f) = &entry.value {
                            inputs.player_cmds.push(PlayerCommand::Seek {
                                player: player_id,
                                time: *f,
                            });
                        }
                    }
                    _ => {
                        // Unknown action - skip for now
                    }
                }
                continue;
            }

            // Instance update path: anim/player/<pid>/instance/<iid>/<field>
            if segs.len() >= 6 && segs[3] == "instance" {
                let inst_id = match Self::parse_u32_segment(segs[4]) {
                    Some(n) => InstId(n),
                    None => continue,
                };
                let field = segs[5];
                // Prepare a base InstanceUpdate with player+inst and defaults None
                let mut upd = InstanceUpdate {
                    player: player_id,
                    inst: inst_id,
                    weight: None,
                    time_scale: None,
                    start_offset: None,
                    enabled: None,
                };
                match field {
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
                continue;
            }
        }

        inputs
    }

    /// Update the animation controller for dt seconds, reading relevant inputs from the
    /// blackboard and returning a WriteBatch plus a list of high-level event values.
    ///
    /// Behavior:
    ///  - Build `Inputs` from the Blackboard using a small path convention
    ///  - Call `engine.update_values(dt, inputs)` to advance the engine and collect Outputs
    ///  - Translate `Outputs.changes` into a WriteBatch by parsing change keys into `TypedPath`
    ///    and creating `WriteOp`s. Keys that do not parse to a TypedPath are skipped.
    ///  - Serialize engine events into `serde_json::Value` and return them alongside the batch.
    pub fn update(&mut self, dt: f32, bb: &mut Blackboard) -> Result<(WriteBatch, Vec<JsonValue>)> {
        // Build Inputs from Blackboard
        let inputs = Self::map_blackboard_to_inputs(bb);

        // Step engine and get outputs reference
        let outputs = self.engine.update_values(dt, inputs);

        // Build WriteBatch from engine outputs (skip non-typed keys)
        let mut batch = WriteBatch::new();
        for ch in outputs.changes.iter() {
            if let Ok(tp) = TypedPath::parse(&ch.key) {
                batch.push(WriteOp::new(tp, ch.value.clone()));
            }
        }

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
    #[serde(default)]
    loop_mode: Option<String>,
    #[serde(default)]
    speed: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
struct InstanceSetup {
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default)]
    time_scale: Option<f32>,
    #[serde(default)]
    start_offset: Option<f32>,
    #[serde(default)]
    enabled: Option<bool>,
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

        let inputs = AnimationController::map_blackboard_to_inputs(&bb);
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
}
