use js_sys::{Function, JSON};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

use vizij_animation_core::{
    parse_stored_animation_json, AnimId, AnimationData, Config, Engine, Inputs, InstId,
    InstanceCfg, Outputs, PlayerId, TargetResolver,
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
    #[wasm_bindgen]
    pub fn update(&mut self, dt: f32, inputs_json: JsValue) -> Result<JsValue, JsError> {
        let inputs: Inputs = if jsvalue_is_undefined_or_null(&inputs_json) {
            Inputs::default()
        } else {
            swb::from_value(inputs_json).map_err(|e| JsError::new(&format!("inputs error: {e}")))?
        };
        let out: &Outputs = self.core.update(dt, inputs);
        swb::to_value(out).map_err(|e| JsError::new(&format!("outputs error: {e}")))
    }
}

/// Numeric ABI version for compatibility checks at init.
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    1
}
