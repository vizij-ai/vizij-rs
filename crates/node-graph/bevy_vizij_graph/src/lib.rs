use bevy::prelude::*;
use hashbrown::HashMap;
use vizij_graph_core::{evaluate_all, GraphRuntime, GraphSpec, NodeId, NodeParams, Value};

#[derive(Resource, Default, Clone)]
pub struct GraphResource(pub GraphSpec);

#[derive(Resource, Default, Clone)]
pub struct GraphOutputs(pub HashMap<NodeId, Value>);

#[derive(Event)]
pub struct SetNodeParam {
    pub node: NodeId,
    pub key: String,
    pub value: Value,
}

#[derive(Resource, Default)]
pub struct GraphTime {
    pub t: f64,
}

pub struct VizijGraphPlugin;

impl Plugin for VizijGraphPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GraphResource::default())
            .insert_resource(GraphOutputs::default())
            .insert_resource(GraphTime { t: 0.0 })
            .add_event::<SetNodeParam>()
            .add_systems(
                Update,
                (system_time, system_set_params, system_eval).chain(),
            );
    }
}

fn system_time(time: Res<Time>, mut gt: ResMut<GraphTime>) {
    gt.t += time.delta_seconds_f64();
}

fn system_set_params(mut ev: EventReader<SetNodeParam>, mut g: ResMut<GraphResource>) {
    for e in ev.read() {
        if let Some(node) = g.0.nodes.iter_mut().find(|n| n.id == e.node) {
            match e.key.as_str() {
                "value" => node.params.value = Some(e.value),
                "frequency" => {
                    node.params.frequency = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "phase" => {
                    node.params.phase = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "min" => {
                    node.params.min = match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    }
                }
                "max" => {
                    node.params.max = match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    }
                }
                "in_min" => {
                    node.params.in_min = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "in_max" => {
                    node.params.in_max = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "out_min" => {
                    node.params.out_min = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "out_max" => {
                    node.params.out_max = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "x" => {
                    node.params.x = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "y" => {
                    node.params.y = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[1],
                    })
                }
                "z" => {
                    node.params.z = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[2],
                    })
                }
                "bone1" => {
                    node.params.bone1 = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "bone2" => {
                    node.params.bone2 = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "bone3" => {
                    node.params.bone3 = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                "index" => {
                    node.params.index = Some(match e.value {
                        Value::Float(f) => f,
                        Value::Bool(b) => {
                            if b {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        Value::Vec3(v) => v[0],
                    })
                }
                _ => { /* ignore unknown keys */ }
            }
        }
    }
}

fn system_eval(g: Res<GraphResource>, mut out: ResMut<GraphOutputs>, gt: Res<GraphTime>) {
    let mut rt = GraphRuntime {
        t: gt.t,
        outputs: HashMap::new(),
    };
    let _ = evaluate_all(&mut rt, &g.0);
    out.0 = rt.outputs;
}
