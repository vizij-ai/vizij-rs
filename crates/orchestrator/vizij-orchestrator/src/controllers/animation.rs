use anyhow::Result;
use serde_json::Value as JsonValue;

use vizij_animation_core::{
    config::Config,
    ids::{InstId, PlayerId},
    inputs::{Inputs, InstanceUpdate, PlayerCommand},
    Engine,
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
    pub fn new(cfg: AnimationControllerConfig) -> Self {
        // Create engine with a default config for now. `setup` can be parsed later to customize.
        let engine = Engine::new(Config::default());
        Self { id: cfg.id, engine }
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
