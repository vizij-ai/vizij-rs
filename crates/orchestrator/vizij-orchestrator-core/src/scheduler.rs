use anyhow::Result;
use std::time::Instant;

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
    let start = Instant::now();
    let mut timings = std::collections::HashMap::new();
    let mut conflicts_out: Vec<JsonValue> = Vec::new();
    let mut events_out: Vec<JsonValue> = Vec::new();

    // Animations phase
    let t0 = Instant::now();
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
        for c in conflict_logs {
            if let Ok(v) = serde_json::to_value(&c) {
                conflicts_out.push(v);
            }
        }
        // collect events (animation-level)
        for e in events {
            events_out.push(e);
        }
    }
    let t_anim = t0.elapsed();
    timings.insert("animations_ms".to_string(), t_anim.as_secs_f32() * 1000.0);

    // Graphs phase
    let t1 = Instant::now();
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
        for c in conflict_logs {
            if let Ok(v) = serde_json::to_value(&c) {
                conflicts_out.push(v);
            }
        }
    }
    let t_graph = t1.elapsed();
    timings.insert("graphs_ms".to_string(), t_graph.as_secs_f32() * 1000.0);

    let total = start.elapsed();
    timings.insert("total_ms".to_string(), total.as_secs_f32() * 1000.0);

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
    let start = Instant::now();
    let mut timings = std::collections::HashMap::new();
    let mut conflicts_out: Vec<JsonValue> = Vec::new();
    let mut events_out: Vec<JsonValue> = Vec::new();

    // First graphs pass
    let t0 = Instant::now();
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
        for c in conflict_logs {
            if let Ok(v) = serde_json::to_value(&c) {
                conflicts_out.push(v);
            }
        }
    }
    let t_graph1 = t0.elapsed();
    timings.insert(
        "graphs_pass1_ms".to_string(),
        t_graph1.as_secs_f32() * 1000.0,
    );

    // Animations pass
    let t1 = Instant::now();
    for (id, anim) in orchestrator.anims.iter_mut() {
        let (batch, events) = anim.update(dt, &mut orchestrator.blackboard)?;
        // accumulate merged writes
        merged_writes.append(batch.clone());
        let conflict_logs: Vec<ConflictLog> = orchestrator
            .blackboard
            .apply_writebatch(batch, orchestrator.epoch, format!("anim:{}", id))
            .into_iter()
            .collect();
        for c in conflict_logs {
            if let Ok(v) = serde_json::to_value(&c) {
                conflicts_out.push(v);
            }
        }
        for e in events {
            events_out.push(e);
        }
    }
    let t_anim = t1.elapsed();
    timings.insert("animations_ms".to_string(), t_anim.as_secs_f32() * 1000.0);

    // Second graphs pass (to pick up animation-produced writes)
    let t2 = Instant::now();
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
        for c in conflict_logs {
            if let Ok(v) = serde_json::to_value(&c) {
                conflicts_out.push(v);
            }
        }
    }
    let t_graph2 = t2.elapsed();
    timings.insert(
        "graphs_pass2_ms".to_string(),
        t_graph2.as_secs_f32() * 1000.0,
    );

    let total = start.elapsed();
    timings.insert("total_ms".to_string(), total.as_secs_f32() * 1000.0);

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
