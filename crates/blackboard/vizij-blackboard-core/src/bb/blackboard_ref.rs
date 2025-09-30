#![allow(unused_must_use)]

use crate::arc_arora_blackboard::{ArcAroraBlackboard, JsonSerializable};
use crate::bb::{
    ABBNodeTrait, ArcABBNode, ArcABBPathNodeTrait, ArcAroraBlackboardTrait,
    ArcNamespacedSetterTrait, ItemsFormattable,
};
use crate::simple_blackboard::{ItemHolder, SimpleBlackboard};
use arora_schema::gen_bb_uuid;
use arora_schema::keyvalue::KeyValue;
use arora_schema::value::Value;
use serde_json::Value as JsonValue;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// Define blackboard adapters to provide a consistent interface
#[derive(Copy, Clone)]
pub enum BlackboardType {
    Simple,
    ArcArora,
    // Add more blackboard types here as needed:
    // YourNewBlackboard,
}

impl BlackboardType {
    // Returns a list of all available blackboard types
    pub fn all_types() -> Vec<BlackboardType> {
        vec![
            BlackboardType::Simple,
            BlackboardType::ArcArora,
            // Add more blackboard types here as needed
        ]
    }

    // Returns the name of this blackboard type for display
    pub fn name(&self) -> &'static str {
        match self {
            BlackboardType::Simple => "SimpleBlackboard",
            BlackboardType::ArcArora => "ArcAroraBlackboard",
            // Add more blackboard types here as needed
        }
    }
}

pub trait BlackboardInterface {
    fn get_name(&self) -> Result<String, String>;
    fn print(&self, tree_only: bool) -> Result<(), String>;
    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Uuid, String>;
    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        id: &Uuid,
    ) -> Result<Uuid, String>;
    fn set_by_id(&mut self, id: &Uuid, value: Value) -> Result<Uuid, String>;
    fn lookup<S: ToString + ?Sized>(&self, path: &S) -> Option<Value>;
    fn lookup_by_id(&self, id: &Uuid) -> Option<Value>;
    fn lookup_kv_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String>;
    fn to_json(&self) -> Result<JsonValue, String>;
    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcABBNode>>>;
    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcABBNode>>>;
}

// This struct provides a unified interface to different blackboard implementations
pub struct BlackboardRef {
    bb_type: BlackboardType,
    simple_bb: Option<SimpleBlackboard>,
    arc_arora_bb: Option<Arc<Mutex<ArcAroraBlackboard>>>,
}

impl BlackboardRef {
    pub fn new<S: ToString + ?Sized>(bb_type: BlackboardType, name: &S) -> Self {
        match bb_type {
            BlackboardType::Simple => BlackboardRef {
                bb_type,
                simple_bb: Some(SimpleBlackboard::new(name.to_string())),
                arc_arora_bb: None,
            },
            BlackboardType::ArcArora => BlackboardRef {
                bb_type,
                simple_bb: None,
                arc_arora_bb: Some(ArcAroraBlackboard::new(&name.to_string())),
            },
        }
    }

    fn debug_message(&self, message: &str) {
        println!("BlackboardRef Debug: {}", message);
    }
}

impl BlackboardInterface for BlackboardRef {
    fn to_json(&self) -> Result<JsonValue, String> {
        match self.bb_type {
            BlackboardType::Simple => {
                unimplemented!("to_json not implemented for SimpleBlackboard")
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    // Use the JsonSerializable trait
                    bb.to_json()
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        }
    }

    fn get_name(&self) -> Result<String, String> {
        match self.bb_type {
            BlackboardType::Simple => {
                if let Some(bb) = &self.simple_bb {
                    Ok(bb.name().to_string())
                } else {
                    Err("SimpleBlackboard is not initialized".to_string())
                }
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    Ok(bb.get_current_name_copy().map_err(|e| e.to_string())?)
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        }
    }

    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Uuid, String> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                if let Some(bb) = &mut self.simple_bb {
                    bb.add_item(path, value);
                    Ok(gen_bb_uuid())
                } else {
                    Err("SimpleBlackboard is not initialized".to_string())
                }
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set(path, value)
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        if let Err(e) = res {
            let error_msg = format!("Failed to insert into blackboard: {}", e);
            self.debug_message(&error_msg);
            Err(error_msg)
        } else {
            res
        }
    }

    fn lookup<S: ToString + ?Sized>(&self, path: &S) -> Option<Value> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                if let Some(bb) = &self.simple_bb {
                    Ok(bb.get_item(path).cloned())
                } else {
                    Err("SimpleBlackboard is not initialized".to_string())
                }
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    let item = bb.get(&path.to_string());
                    if let Ok(ok_item) = &item {
                        if let Some(some_item) = ok_item {
                            let guard: std::sync::MutexGuard<'_, ArcABBNode> =
                                some_item.lock().unwrap();
                            if let Ok(is_path) = guard.is_path() {
                                if is_path {
                                    let path_id =
                                        guard.get_id_copy().expect("Path ID should exist");
                                    drop(guard); // Explicitly unlock the MutexGuard before further operations
                                    let path_kv =
                                        self.lookup_kv_by_id(&path_id).map_err(|e| e.to_string());
                                    if path_kv.is_ok() {
                                        let kv = path_kv.unwrap();
                                        Ok(if let Some(kv) = kv {
                                            Some(Value::KeyValue(kv))
                                        } else {
                                            None
                                        })
                                    } else {
                                        Err(format!(
                                            "Failed to get KeyValue for path '{}': {}",
                                            path.to_string(),
                                            path_kv.err().unwrap_or("Unknown error".to_string())
                                        ))
                                    }
                                } else {
                                    Ok(guard
                                        .as_item()
                                        .and_then(|path_node| path_node.get_value().cloned()))
                                }
                            } else {
                                Err(format!(
                                    "Failed to check if node '{}' is a path",
                                    path.to_string()
                                ))
                            }
                        } else {
                            Err(format!(
                                "Item '{}' not found in ArcAroraBlackboard",
                                path.to_string()
                            ))
                        }
                    } else {
                        Err(format!(
                            "Failed to get item '{}' from ArcAroraBlackboard: {:?}",
                            path.to_string(),
                            item.err().unwrap_or("Unknown error".to_string())
                        ))
                    }
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        match res {
            Ok(value) => value,
            Err(e) => {
                let error_msg = format!("Failed to lookup from blackboard: {}", e);
                self.debug_message(&error_msg);
                None
            }
        }
    }

    fn lookup_kv_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                unimplemented!("get_keyvalue_by_id not implemented for SimpleBlackboard")
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    bb.get_keyvalue_by_id(&id)
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        match res {
            Ok(node) => Ok(node),
            Err(e) => {
                let error_msg = format!("Failed to get node by ID from blackboard: {}", e);
                self.debug_message(&error_msg);
                Err(error_msg)
            }
        }
    }

    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcABBNode>>> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                unimplemented!("SimpleBlackboard does not support node lookup")
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    let arc_node = bb
                        .get(path)
                        .expect("Failed to get node from ArcAroraBlackboard");
                    match arc_node {
                        Some(node) => Ok(node),
                        None => Err(format!(
                            "Node '{}' not found in ArcAroraBlackboard",
                            path.to_string()
                        )),
                    }
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        res.ok()
    }

    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcABBNode>>> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                unimplemented!("SimpleBlackboard does not support node lookup by ID")
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    bb.get_node_by_id(&id).map_err(|e| e.to_string())
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        match res {
            Ok(node) => node,
            Err(e) => {
                let error_msg = format!("Failed to lookup node by ID: {}", e);
                self.debug_message(&error_msg);
                None
            }
        }
    }

    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        id: &Uuid,
    ) -> Result<Uuid, String> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                if let Some(bb) = &mut self.simple_bb {
                    bb.add_item(path, value);
                    Ok(gen_bb_uuid())
                } else {
                    Err("SimpleBlackboard is not initialized".to_string())
                }
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set_with_id(&path.to_string(), value, Some(*id))
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        if let Err(e) = res {
            let error_msg = format!("Failed to set_with_id in blackboard: {}", e);
            self.debug_message(&error_msg);
            Err(error_msg)
        } else {
            res
        }
    }

    fn set_by_id(&mut self, id: &Uuid, value: Value) -> Result<Uuid, String> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                unimplemented!("set_by_id not implemented for SimpleBlackboard")
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set_existing_bb_item(value, &id)
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        if let Err(e) = res {
            let error_msg = format!("Failed to set_by_id in blackboard: {}", e);
            self.debug_message(&error_msg);
            Err(error_msg)
        } else {
            Ok(*id)
        }
    }

    fn lookup_by_id(&self, id: &Uuid) -> Option<Value> {
        let res = match self.bb_type {
            BlackboardType::Simple => {
                if let Some(bb) = &self.simple_bb {
                    Ok(bb.get_item(id).cloned())
                } else {
                    Err("SimpleBlackboard is not initialized".to_string())
                }
            }
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    match bb.get_node_by_id(&id) {
                        Ok(Some(node)) => {
                            let target_node = node.lock().unwrap();
                            match target_node.is_path() {
                                Ok(true) => {
                                    Err("Cannot get value from a path node by ID".to_string())
                                }
                                Ok(false) => Ok(target_node.as_item()?.get_value().cloned()),
                                Err(e) => Err(e),
                            }
                        }
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    }
                } else {
                    Err("ArcAroraBlackboard is not initialized".to_string())
                }
            }
        };
        match res {
            Ok(value) => value,
            Err(e) => {
                let error_msg = format!("Failed to lookup_with_id from blackboard: {}", e);
                self.debug_message(&error_msg);
                None
            }
        }
    }

    fn print(&self, tree_only: bool) -> Result<(), String> {
        match self.bb_type {
            BlackboardType::Simple => unimplemented!("Print not implemented for SimpleBlackboard"),
            BlackboardType::ArcArora => {
                if let Some(bb) = &self.arc_arora_bb {
                    let bb_lock = bb.lock().unwrap();
                    if !tree_only {
                        println!("{}", bb_lock.format_items(true));
                    }
                    println!("{}", bb_lock.format_tree(true));
                } else {
                    println!("ArcAroraBlackboard is not initialized.");
                }
                Ok(())
            }
        }
    }
}

impl BlackboardInterface for Arc<Mutex<BlackboardRef>> {
    fn print(&self, tree_only: bool) -> Result<(), String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .print(tree_only);
        Ok(())
    }

    fn get_name(&self) -> Result<String, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .get_name()
    }

    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Uuid, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .set(path, value)
    }

    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        id: &Uuid,
    ) -> Result<Uuid, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .set_with_id(path, value, id)
    }

    fn set_by_id(&mut self, id: &Uuid, value: Value) -> Result<Uuid, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .set_by_id(id, value)
    }

    fn lookup<S: ToString + ?Sized>(&self, path: &S) -> Option<Value> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup(path))
    }

    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcABBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_node(path))
    }

    fn lookup_by_id(&self, id: &Uuid) -> Option<Value> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_by_id(id))
    }

    fn lookup_kv_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .lookup_kv_by_id(id)
    }

    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcABBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_node_by_id(id))
    }

    fn to_json(&self) -> Result<JsonValue, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .to_json()
    }
}
