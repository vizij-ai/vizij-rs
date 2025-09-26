use js_sys::{Function, JSON};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

use serde_json::{json, to_value, Map};
use vizij_animation_core::{
    parse_stored_animation_json, AnimId, AnimationData, BakingConfig, Config, Engine, Inputs,
    InstId, InstanceCfg, Outputs, OutputsWithDerivatives, PlayerId, TargetResolver,
};

#[wasm_bindgen]
pub struct VizijAnimation {
    core: Engine,
}

fn jsvalue_is_undefined_or_null(v: &JsValue) -> bool {
    v.is_undefined() || v.is_null()
}

struct JsResolver {
    f: Function,
}

impl TargetResolver for JsResolver {
    fn resolve(&mut self, path: &str) -> Option<String> {
        // Call JS resolver(path) - expect string key; allow number fallback -> string
        let arg = JsValue::from_str(path);
        match self.f.call1(&JsValue::UNDEFINED, &arg) {
            Ok(val) => {
                if val.is_undefined() || val.is_null() {
                    return None;
                }
                if let Some(s) = val.as_string() {
                    return Some(s);
                }
                if let Some(n) = val.as_f64() {
                    return Some(if n.fract() == 0.0 {
                        format!("{}", n as i64)
                    } else {
                        format!("{}", n)
                    });
                }
                // Attempt serde conversion to String as a last resort
                swb::from_value::<String>(val).ok()
            }
            Err(_) => None,
        }
    }
}

#[wasm_bindgen]
impl VizijAnimation {
    /// Create a new engine instance. Pass a JSON config object or undefined/null for defaults.
    /// Example:
    ///   new VizijAnimation({ scratch_samples: 2048 })
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<VizijAnimation, JsError> {
        console_error_panic_hook::set_once();

        let cfg: Config = if jsvalue_is_undefined_or_null(&config) {
            Config::default()
        } else {
            swb::from_value(config).map_err(|e| JsError::new(&format!("config error: {e}")))?
        };

        Ok(VizijAnimation {
            core: Engine::new(cfg),
        })
    }

    /// Load an AnimationData (JSON) into the engine. Returns an AnimId (u32).
    #[wasm_bindgen(js_name = load_animation)]
    pub fn load_animation(&mut self, data_json: JsValue) -> Result<u32, JsError> {
        let data: AnimationData = swb::from_value(data_json)
            .map_err(|e| JsError::new(&format!("load_animation parse error: {e}")))?;
        let id: AnimId = self.core.load_animation(data);
        Ok(id.0)
    }

    /// Load a StoredAnimation JSON (new format: tracks with keypoints and transitions.in/out) into the engine.
    /// Accepts any JS object compatible with tests/fixtures/new_format.json. Returns an AnimId (u32).
    #[wasm_bindgen(js_name = load_stored_animation)]
    pub fn load_stored_animation(&mut self, data_json: JsValue) -> Result<u32, JsError> {
        if jsvalue_is_undefined_or_null(&data_json) {
            return Err(JsError::new(
                "load_stored_animation: data_json is null/undefined",
            ));
        }
        // Stringify the JS object so we can reuse the core parser (expects &str)
        let s = JSON::stringify(&data_json)
            .map_err(|e| JsError::new(&format!("load_stored_animation stringify error: {:?}", e)))?
            .as_string()
            .ok_or_else(|| JsError::new("load_stored_animation: stringify produced non-string"))?;
        let data = parse_stored_animation_json(&s)
            .map_err(|e| JsError::new(&format!("load_stored_animation parse error: {e}")))?;
        let id: AnimId = self.core.load_animation(data);
        Ok(id.0)
    }

    /// Create a new player by display name. Returns a PlayerId (u32).
    #[wasm_bindgen(js_name = create_player)]
    pub fn create_player(&mut self, name: String) -> u32 {
        let pid: PlayerId = self.core.create_player(&name);
        pid.0
    }

    /// Add an animation instance to a player. `cfg` is optional JSON matching InstanceCfg.
    /// Returns an InstId (u32).
    #[wasm_bindgen(js_name = add_instance)]
    pub fn add_instance(
        &mut self,
        player_id: u32,
        anim_id: u32,
        cfg: JsValue,
    ) -> Result<u32, JsError> {
        let cfg_rs: InstanceCfg = if jsvalue_is_undefined_or_null(&cfg) {
            InstanceCfg::default()
        } else {
            swb::from_value(cfg).map_err(|e| JsError::new(&format!("instance cfg error: {e}")))?
        };
        let pid = PlayerId(player_id);
        let aid = AnimId(anim_id);
        let iid: InstId = self.core.add_instance(pid, aid, cfg_rs);
        Ok(iid.0)
    }

    /// Resolve canonical target paths to opaque keys using a JS resolver callback.
    /// The resolver is called as `resolver(path: string) -> string | number | null/undefined`.
    /// Resolved values are stored as strings.
    #[wasm_bindgen]
    pub fn prebind(&mut self, resolver: Function) {
        let mut js_resolver = JsResolver { f: resolver };
        self.core.prebind(&mut js_resolver);
    }

    /// Step the simulation by dt (seconds) with inputs JSON. Returns Outputs JSON.
    #[wasm_bindgen(js_name = update_values)]
    pub fn update_values(&mut self, dt: f32, inputs_json: JsValue) -> Result<JsValue, JsError> {
        let inputs: Inputs = if jsvalue_is_undefined_or_null(&inputs_json) {
            Inputs::default()
        } else {
            swb::from_value(inputs_json).map_err(|e| JsError::new(&format!("inputs error: {e}")))?
        };
        let out: &Outputs = self.core.update_values(dt, inputs);
        swb::to_value(out).map_err(|e| JsError::new(&format!("outputs error: {e}")))
    }

    /// Step the simulation by dt (seconds) and return Outputs including derivatives JSON.
    #[wasm_bindgen(js_name = update_values_with_derivatives)]
    pub fn update_values_with_derivatives(
        &mut self,
        dt: f32,
        inputs_json: JsValue,
    ) -> Result<JsValue, JsError> {
        let inputs: Inputs = if jsvalue_is_undefined_or_null(&inputs_json) {
            Inputs::default()
        } else {
            swb::from_value(inputs_json).map_err(|e| JsError::new(&format!("inputs error: {e}")))?
        };
        let out: &OutputsWithDerivatives = self.core.update_with_derivatives(dt, inputs);
        swb::to_value(out).map_err(|e| JsError::new(&format!("outputs error: {e}")))
    }

    /// Backwards-compatible alias for update_values.
    #[wasm_bindgen]
    pub fn update(&mut self, dt: f32, inputs_json: JsValue) -> Result<JsValue, JsError> {
        self.update_values(dt, inputs_json)
    }

    /// Step the simulation and return a nodes+writes JSON object compatible with
    /// the node-graph WASM output shape.
    /// Returns an object with shape:
    /// { nodes: Record<string, Record<string, ValueJSON>>, writes: Array<{ path: string, value: ValueJSON }> }.
    #[wasm_bindgen(js_name = update_nodes_writes)]
    pub fn update_nodes_writes(
        &mut self,
        dt: f32,
        inputs_json: JsValue,
    ) -> Result<JsValue, JsError> {
        let inputs: Inputs = if jsvalue_is_undefined_or_null(&inputs_json) {
            Inputs::default()
        } else {
            swb::from_value(inputs_json).map_err(|e| JsError::new(&format!("inputs error: {e}")))?
        };

        // Produce a typed WriteBatch from the engine (skips keys that don't parse).
        let batch = self.core.update_writebatch(dt, inputs);

        // Serialize the WriteBatch into JSON (an array of { path, value } objects).
        let writes_json =
            to_value(&batch).map_err(|e| JsError::new(&format!("serialize batch error: {e}")))?;

        // For animation the per-node outputs map is not applicable; keep empty object for tooling parity.
        let out_obj = json!({
            "nodes": Map::new(),
            "writes": writes_json,
        });

        swb::to_value(&out_obj).map_err(|e| JsError::new(&format!("outputs error: {e}")))
    }

    /// Bake an animation into sampled values using the provided config.
    #[wasm_bindgen(js_name = bake_animation)]
    pub fn bake_animation(&mut self, anim_id: u32, cfg_json: JsValue) -> Result<JsValue, JsError> {
        let cfg: BakingConfig = if jsvalue_is_undefined_or_null(&cfg_json) {
            BakingConfig::default()
        } else {
            swb::from_value(cfg_json)
                .map_err(|e| JsError::new(&format!("bake config error: {e}")))?
        };
        let anim = AnimId(anim_id);
        let data = self
            .core
            .animation_data(anim)
            .ok_or_else(|| JsError::new("bake_animation: unknown animation id"))?;
        let baked = vizij_animation_core::baking::bake_animation_data(anim, data, &cfg);
        swb::to_value(&baked).map_err(|e| JsError::new(&format!("bake serialize error: {e}")))
    }

    /// Bake values and derivatives for an animation in a single call.
    #[wasm_bindgen(js_name = bake_animation_with_derivatives)]
    pub fn bake_animation_with_derivatives(
        &mut self,
        anim_id: u32,
        cfg_json: JsValue,
    ) -> Result<JsValue, JsError> {
        let cfg: BakingConfig = if jsvalue_is_undefined_or_null(&cfg_json) {
            BakingConfig::default()
        } else {
            swb::from_value(cfg_json)
                .map_err(|e| JsError::new(&format!("bake config error: {e}")))?
        };
        let anim = AnimId(anim_id);
        let data = self
            .core
            .animation_data(anim)
            .ok_or_else(|| JsError::new("bake_animation_with_derivatives: unknown animation id"))?;
        let (values, derivatives) =
            vizij_animation_core::baking::bake_animation_data_with_derivatives(anim, data, &cfg);
        let out = json!({
            "values": values,
            "derivatives": derivatives,
        });
        swb::to_value(&out).map_err(|e| JsError::new(&format!("bake serialize error: {e}")))
    }

    /// Remove a player and all its instances. Returns boolean success.
    #[wasm_bindgen(js_name = remove_player)]
    pub fn remove_player(&mut self, player_id: u32) -> bool {
        self.core.remove_player(PlayerId(player_id))
    }

    /// Remove a specific instance from a player. Returns boolean success.
    #[wasm_bindgen(js_name = remove_instance)]
    pub fn remove_instance(&mut self, player_id: u32, inst_id: u32) -> bool {
        self.core
            .remove_instance(PlayerId(player_id), InstId(inst_id))
    }

    /// Unload an animation and detach all referencing instances. Returns boolean success.
    #[wasm_bindgen(js_name = unload_animation)]
    pub fn unload_animation(&mut self, anim_id: u32) -> bool {
        self.core.unload_animation(AnimId(anim_id))
    }

    /// List all animations (id, name, duration_ms, track_count).
    #[wasm_bindgen(js_name = list_animations)]
    pub fn list_animations(&self) -> Result<JsValue, JsError> {
        let v = self.core.list_animations();
        swb::to_value(&v).map_err(|e| JsError::new(&format!("list_animations error: {e}")))
    }

    /// List all players with playback info and computed length.
    #[wasm_bindgen(js_name = list_players)]
    pub fn list_players(&self) -> Result<JsValue, JsError> {
        let v = self.core.list_players();
        swb::to_value(&v).map_err(|e| JsError::new(&format!("list_players error: {e}")))
    }

    /// List all instances for a given player id.
    #[wasm_bindgen(js_name = list_instances)]
    pub fn list_instances(&self, player_id: u32) -> Result<JsValue, JsError> {
        let v = self.core.list_instances(PlayerId(player_id));
        swb::to_value(&v).map_err(|e| JsError::new(&format!("list_instances error: {e}")))
    }

    /// List the set of resolved output keys currently associated with the player's instances.
    #[wasm_bindgen(js_name = list_player_keys)]
    pub fn list_player_keys(&self, player_id: u32) -> Result<JsValue, JsError> {
        let v = self.core.list_player_keys(PlayerId(player_id));
        swb::to_value(&v).map_err(|e| JsError::new(&format!("list_player_keys error: {e}")))
    }
}

/// Numeric ABI version for compatibility checks at init.
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    2
}
