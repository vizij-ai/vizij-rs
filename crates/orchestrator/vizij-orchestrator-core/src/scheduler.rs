use anyhow::Result;
use serde_json::Value as JsonValue;

use vizij_api_core::WriteBatch;

use crate::blackboard::ConflictLog;

/// Scheduling strategies supported by the orchestrator.
#[derive(Debug, Clone, Copy)]
pub enum Schedule {
    SinglePass,
    TwoPass,
    RateDecoupled, // reserved for future work
}

/// Run a single-pass schedule:
///   Animations -> merge -> Graphs -> merge -> frame
pub fn run_single_pass(
    orchestrator: &mut crate::Orchestrator,
    dt: f32,
) -> Result<crate::OrchestratorFrame> {
    let mut timings = std::collections::HashMap::new();
    let mut conflicts_out: Vec<ConflictLog> = Vec::new();
    let mut events_out: Vec<JsonValue> = Vec::new();

    // Animations phase
    let mut merged_writes = WriteBatch::new();
    for (id, anim) in orchestrator.anims.iter_mut() {
        let (batch, events) = anim.update(dt, &mut orchestrator.blackboard)?;
        // accumulate merged writes in pass/controller order
        merged_writes.append(batch.clone());
        // apply batch, record conflicts
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(batch, orchestrator.epoch, format!("anim:{}", id))
            .into_iter()
            .collect();
        conflicts_out.extend(conflict_logs);
        // collect events (animation-level)
        for e in events {
            events_out.push(e);
        }
    }
    if !orchestrator.anims.is_empty() {
        timings.insert("animations_ms".to_string(), dt * 1000.0);
    }

    // Graphs phase
    for (id, graph) in orchestrator.graphs.iter_mut() {
        let batch = graph.evaluate(&mut orchestrator.blackboard, orchestrator.epoch, dt)?;
        // Filter batch according to graph subscriptions: if outputs is empty -> publish all, else only listed paths
        let publish_batch = if graph.subs.outputs.is_empty() {
            batch.clone()
        } else {
            let mut b = WriteBatch::new();
            for op in batch.iter() {
                if graph.subs.outputs.iter().any(|p| p == &op.path) {
                    b.push(op.clone());
                }
            }
            b
        };
        // accumulate merged writes (only published writes)
        merged_writes.append(publish_batch.clone());
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(publish_batch, orchestrator.epoch, format!("graph:{}", id))
            .into_iter()
            .collect();
        conflicts_out.extend(conflict_logs);
    }
    if !orchestrator.graphs.is_empty() {
        timings.insert("graphs_ms".to_string(), dt * 1000.0);
    }

    timings.insert("total_ms".to_string(), dt * 1000.0);

    let frame = crate::OrchestratorFrame {
        epoch: orchestrator.epoch,
        dt,
        merged_writes,
        conflicts: conflicts_out,
        timings_ms: timings,
        events: events_out,
    };

    Ok(frame)
}

/// Run a two-pass schedule:
///   Graphs -> merge -> Animations -> merge -> Graphs -> merge -> frame
pub fn run_two_pass(
    orchestrator: &mut crate::Orchestrator,
    dt: f32,
) -> Result<crate::OrchestratorFrame> {
    let mut timings = std::collections::HashMap::new();
    let mut conflicts_out: Vec<ConflictLog> = Vec::new();
    let mut events_out: Vec<JsonValue> = Vec::new();

    // First graphs pass
    let mut merged_writes = WriteBatch::new();
    for (id, graph) in orchestrator.graphs.iter_mut() {
        let batch = graph.evaluate(&mut orchestrator.blackboard, orchestrator.epoch, dt)?;
        let publish_batch = if graph.subs.outputs.is_empty() {
            batch.clone()
        } else {
            let mut b = WriteBatch::new();
            for op in batch.iter() {
                if graph.subs.outputs.iter().any(|p| p == &op.path) {
                    b.push(op.clone());
                }
            }
            b
        };
        // accumulate merged writes
        merged_writes.append(publish_batch.clone());
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(publish_batch, orchestrator.epoch, format!("graph:{}", id))
            .into_iter()
            .collect();
        conflicts_out.extend(conflict_logs);
    }
    if !orchestrator.graphs.is_empty() {
        timings.insert("graphs_pass1_ms".to_string(), dt * 1000.0);
    }

    // Animations pass
    for (id, anim) in orchestrator.anims.iter_mut() {
        let (batch, events) = anim.update(dt, &mut orchestrator.blackboard)?;
        // accumulate merged writes
        merged_writes.append(batch.clone());
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(batch, orchestrator.epoch, format!("anim:{}", id))
            .into_iter()
            .collect();
        conflicts_out.extend(conflict_logs);
        for e in events {
            events_out.push(e);
        }
    }
    if !orchestrator.anims.is_empty() {
        timings.insert("animations_ms".to_string(), dt * 1000.0);
    }

    // Second graphs pass (to pick up animation-produced writes)
    for (id, graph) in orchestrator.graphs.iter_mut() {
        let batch = graph.evaluate(&mut orchestrator.blackboard, orchestrator.epoch, dt)?;
        let publish_batch = if graph.subs.outputs.is_empty() {
            batch.clone()
        } else {
            let mut b = WriteBatch::new();
            for op in batch.iter() {
                if graph.subs.outputs.iter().any(|p| p == &op.path) {
                    b.push(op.clone());
                }
            }
            b
        };
        // accumulate merged writes
        merged_writes.append(publish_batch.clone());
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(publish_batch, orchestrator.epoch, format!("graph:{}", id))
            .into_iter()
            .collect();
        conflicts_out.extend(conflict_logs);
    }
    if !orchestrator.graphs.is_empty() {
        timings.insert("graphs_pass2_ms".to_string(), dt * 1000.0);
    }

    timings.insert("total_ms".to_string(), dt * 1000.0);

    let frame = crate::OrchestratorFrame {
        epoch: orchestrator.epoch,
        dt,
        merged_writes,
        conflicts: conflicts_out,
        timings_ms: timings,
        events: events_out,
    };

    Ok(frame)
}
