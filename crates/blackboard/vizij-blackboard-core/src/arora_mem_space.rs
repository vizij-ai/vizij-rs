#![allow(unused_must_use)]

use crate::arc_bb::{ArcBBNode, ArcBBPathNodeTrait, ArcNamespacedSetterTrait};
use crate::rc_bb::{NamespacedSetterTrait, RcBBNode, RcBBPathNodeTrait};
use crate::traits::{BBNodeTrait, BlackboardTrait, JsonSerializable};
use crate::ArcBlackboard;
use crate::RcBlackboard;

use arora_schema::keyvalue::KeyValue;
use arora_schema::value::Value;
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// Define blackboard adapters to provide a consistent interface
#[derive(Copy, Clone)]
pub enum AroraMemSpaceType {
    Rc,
    Arc,
    // Add more blackboard types here as needed:
    // YourNewBlackboard,
}

impl AroraMemSpaceType {
    // Returns a list of all available blackboard types
    pub fn all_types() -> Vec<AroraMemSpaceType> {
        vec![
            AroraMemSpaceType::Rc,
            AroraMemSpaceType::Arc,
            // Add more blackboard types here as needed
        ]
    }

    // Returns the name of this blackboard type for display
    pub fn name(&self) -> &'static str {
        match self {
            AroraMemSpaceType::Rc => "RcBlackboard",
            AroraMemSpaceType::Arc => "ArcBlackboard",
            // Add more blackboard types here as needed
        }
    }
}

pub trait AroraMemSpaceInterface {
    fn get_name(&self) -> Result<String, String>;
    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Vec<Uuid>, String>;
    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        id: &Uuid,
    ) -> Result<Vec<Uuid>, String>;
    fn set_by_id(&mut self, id: &Uuid, value: Value) -> Result<Vec<Uuid>, String>;
    fn lookup<S: ToString + ?Sized>(&self, path: &S) -> Option<Value>;
    fn lookup_by_id(&self, id: &Uuid) -> Option<Value>;
    fn to_json(&self) -> Result<JsonValue, String>;
    fn remove<S: ToString + ?Sized>(&mut self, path: &S) -> Result<Vec<Uuid>, String>;
    fn remove_by_id(&mut self, id: &Uuid) -> Result<Vec<Uuid>, String>;
}

/// Thread-safe access helpers that expose raw node handles.
pub trait AMSNodeAccess {
    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Rc<RefCell<RcBBNode>>>;
    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Rc<RefCell<RcBBNode>>>;
    fn lookup_arc_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcBBNode>>>;
    fn lookup_arc_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcBBNode>>>;
}

// This struct provides a unified interface to different blackboard implementations
pub struct AroraMemSpace {
    ams_type: AroraMemSpaceType,
    arora_bb: Option<Rc<RefCell<RcBlackboard>>>,
    arc_arora_bb: Option<Arc<Mutex<ArcBlackboard>>>,
}

impl AroraMemSpace {
    pub fn new<S: ToString + ?Sized>(bb_type: AroraMemSpaceType, name: &S) -> Self {
        match bb_type {
            AroraMemSpaceType::Rc => AroraMemSpace {
                ams_type: bb_type,
                arora_bb: Some(RcBlackboard::new(name.to_string())),
                arc_arora_bb: None,
            },
            AroraMemSpaceType::Arc => AroraMemSpace {
                ams_type: bb_type,
                arora_bb: None,
                arc_arora_bb: Some(ArcBlackboard::new(name.to_string())),
            },
        }
    }

    fn debug_message(&self, message: &str) {
        println!("AMS Debug: {}", message);
    }

    fn _lookup_kv_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    bb.borrow().get_keyvalue_by_id(id)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    bb.get_keyvalue_by_id(id)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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
}

impl AroraMemSpaceInterface for AroraMemSpace {
    fn to_json(&self) -> Result<JsonValue, String> {
        match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    bb.borrow().to_json()
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    // Use the JsonSerializable trait
                    bb.to_json()
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
                }
            }
        }
    }

    fn get_name(&self) -> Result<String, String> {
        match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    Ok(bb
                        .borrow()
                        .get_current_name_copy()
                        .map_err(|e| e.to_string())?)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    Ok(bb.get_current_name_copy().map_err(|e| e.to_string())?)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
                }
            }
        }
    }

    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Uuid, String> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &mut self.arora_bb {
                    bb.borrow_mut().set(path, value)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set(path, value)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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
        let res: Result<Option<Value>, String> = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    let item = bb.borrow().get(&path.to_string());
                    if let Ok(ok_item) = &item {
                        if let Some(some_item_ref) = ok_item {
                            let some_item = some_item_ref.borrow();
                            if let Ok(is_path) = some_item.is_path() {
                                if is_path {
                                    let path_id =
                                        some_item.get_id_copy().expect("Path ID should exist");
                                    // Drop borrows before calling lookup_kv_by_id to avoid double-borrow
                                    drop(some_item);
                                    drop(item);
                                    match self._lookup_kv_by_id(&path_id) {
                                        Ok(opt) => Ok(opt.map(Value::KeyValue)),
                                        Err(e) => Err(format!(
                                            "Failed to get KeyValue for path '{}': {}",
                                            path.to_string(),
                                            e
                                        )),
                                    }
                                } else {
                                    Ok(some_item
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
                                "Item '{}' not found in RcBlackboard",
                                path.to_string()
                            ))
                        }
                    } else {
                        Err(format!(
                            "Failed to get item '{}' from RcBlackboard: {:?}",
                            path.to_string(),
                            item.err().unwrap_or("Unknown error".to_string())
                        ))
                    }
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    let item = { bb.get(&path.to_string()) };
                    if let Ok(ok_item) = &item {
                        if let Some(some_item) = ok_item {
                            let guard: std::sync::MutexGuard<'_, ArcBBNode> =
                                some_item.lock().unwrap();
                            if let Ok(is_path) = guard.is_path() {
                                if is_path {
                                    let path_id =
                                        guard.get_id_copy().expect("Path ID should exist");
                                    drop(guard); // Explicitly unlock the MutexGuard before further operations
                                    match self._lookup_kv_by_id(&path_id) {
                                        Ok(opt) => Ok(opt.map(Value::KeyValue)),
                                        Err(e) => Err(format!(
                                            "Failed to get KeyValue for path '{}': {}",
                                            path.to_string(),
                                            e
                                        )),
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
                                "Item '{}' not found in ArcBlackboard",
                                path.to_string()
                            ))
                        }
                    } else {
                        Err(format!(
                            "Failed to get item '{}' from ArcBlackboard: {:?}",
                            path.to_string(),
                            item.err().unwrap_or("Unknown error".to_string())
                        ))
                    }
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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

    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        id: &Uuid,
    ) -> Result<Uuid, String> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &mut self.arora_bb {
                    bb.borrow_mut()
                        .set_with_id(&path.to_string(), value, Some(*id))
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set_with_id(&path.to_string(), value, Some(*id))
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &mut self.arora_bb {
                    bb.borrow_mut().set_existing_bb_item(value, id)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.set_existing_bb_item(value, id)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    match bb.borrow().get_node_by_id(id) {
                        Ok(Some(node_ref)) => {
                            let node = node_ref.borrow();
                            match node.is_path() {
                                Ok(true) => {
                                    Err("Cannot get value from a path node by ID".to_string())
                                }
                                Ok(false) => Ok(node.as_item()?.get_value().cloned()),
                                Err(e) => Err(e),
                            }
                        }
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    }
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    let node = { bb.get_node_by_id(id) };
                    match node {
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
                    Err("ArcBlackboard is not initialized".to_string())
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

    fn remove<S: ToString + ?Sized>(&mut self, path: &S) -> Result<Vec<Uuid>, String> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &mut self.arora_bb {
                    bb.borrow_mut().remove_item(path)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.lock()
                        .map_err(|_| "Failed to lock ArcBlackboard".to_string())?
                        .remove_item(path)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
                }
            }
        };
        if let Err(e) = res {
            let error_msg = format!("Failed to remove item from blackboard: {}", e);
            self.debug_message(&error_msg);
            Err(error_msg)
        } else {
            res
        }
    }

    fn remove_by_id(&mut self, id: &Uuid) -> Result<Vec<Uuid>, String> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &mut self.arora_bb {
                    bb.borrow_mut().remove_item_by_id(id)
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &mut self.arc_arora_bb {
                    bb.lock()
                        .map_err(|_| "Failed to lock ArcBlackboard".to_string())?
                        .remove_item_by_id(id)
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
                }
            }
        };
        if let Err(e) = res {
            let error_msg = format!("Failed to remove item by ID from blackboard: {}", e);
            self.debug_message(&error_msg);
            Err(error_msg)
        } else {
            res
        }
    }
}

impl AMSNodeAccess for AroraMemSpace {
    fn lookup_arc_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcBBNode>>> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                unimplemented!("RcBlackboard does not support Arc node lookup")
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    let arc_node = bb.get(path).expect("Failed to get node from ArcBlackboard");
                    match arc_node {
                        Some(node) => Ok(node),
                        None => Err(format!(
                            "Node '{}' not found in ArcBlackboard",
                            path.to_string()
                        )),
                    }
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
                }
            }
        };
        res.ok()
    }

    fn lookup_arc_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcBBNode>>> {
        let res = match self.ams_type {
            AroraMemSpaceType::Rc => {
                unimplemented!("RcBlackboard does not support Arc node lookup by ID")
            }
            AroraMemSpaceType::Arc => {
                if let Some(bb) = &self.arc_arora_bb {
                    bb.get_node_by_id(id).map_err(|e| e.to_string())
                } else {
                    Err("ArcBlackboard is not initialized".to_string())
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

    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Rc<RefCell<RcBBNode>>> {
        let res = match self.ams_type {
            AroraMemSpaceType::Arc => {
                unimplemented!("ArcBlackboard does not support Rc node lookup")
            }
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    let node = bb
                        .borrow()
                        .get(path)
                        .expect("Failed to get node from RcBlackboard");
                    match node {
                        Some(n) => Ok(n),
                        None => Err(format!(
                            "Node '{}' not found in RcBlackboard",
                            path.to_string()
                        )),
                    }
                } else {
                    Err("RcBlackboard is not initialized".to_string())
                }
            }
        };
        res.ok()
    }

    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Rc<RefCell<RcBBNode>>> {
        let res = match self.ams_type {
            AroraMemSpaceType::Arc => {
                unimplemented!("ArcBlackboard does not support Rc node lookup by ID")
            }
            AroraMemSpaceType::Rc => {
                if let Some(bb) = &self.arora_bb {
                    bb.borrow().get_node_by_id(id).map_err(|e| e.to_string())
                } else {
                    Err("RcBlackboard is not initialized".to_string())
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
}

impl AroraMemSpaceInterface for Arc<Mutex<AroraMemSpace>> {
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

    fn lookup_by_id(&self, id: &Uuid) -> Option<Value> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_by_id(id))
    }

    fn to_json(&self) -> Result<JsonValue, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .to_json()
    }

    fn remove<S: ToString + ?Sized>(&mut self, path: &S) -> Result<Vec<Uuid>, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .remove(path)
    }

    fn remove_by_id(&mut self, id: &Uuid) -> Result<Vec<Uuid>, String> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")?
            .remove_by_id(id)
    }
}

impl AMSNodeAccess for Arc<Mutex<AroraMemSpace>> {
    fn lookup_arc_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Arc<Mutex<ArcBBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_arc_node(path))
    }

    fn lookup_arc_node_by_id(&self, id: &Uuid) -> Option<Arc<Mutex<ArcBBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_arc_node_by_id(id))
    }

    fn lookup_node<S: ToString + ?Sized>(&self, path: &S) -> Option<Rc<RefCell<RcBBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_node(path))
    }

    fn lookup_node_by_id(&self, id: &Uuid) -> Option<Rc<RefCell<RcBBNode>>> {
        self.lock()
            .map_err(|_| "Failed to lock the blackboard")
            .ok()
            .and_then(|bb| bb.lookup_node_by_id(id))
    }
}
