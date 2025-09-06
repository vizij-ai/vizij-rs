use wasm_bindgen::prelude::*;
use serde_wasm_bindgen as swb;
use vizij_animation_core::{AnimationConfig, AnimationCore, AnimationInputs, AnimationOutputs};

#[wasm_bindgen]
pub struct Animation {
    core: AnimationCore,
}

#[wasm_bindgen]
impl Animation {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<Animation, JsError> {
        console_error_panic_hook::set_once();
        let cfg: AnimationConfig = swb::from_value(config)
            .map_err(|e| JsError::new(&format!("config error: {e}")))?;
        Ok(Animation { core: AnimationCore::new(cfg) })
    }

    /// Step the animation with a time delta (seconds) and inputs.
    #[wasm_bindgen]
    pub fn update(&mut self, dt: f32, inputs: JsValue) -> Result<JsValue, JsError> {
        let inp: AnimationInputs = swb::from_value(inputs)
            .map_err(|e| JsError::new(&format!("inputs error: {e}")))?;
        let out: AnimationOutputs = self.core.update(dt, inp);
        swb::to_value(&out).map_err(|e| JsError::new(&format!("outputs error: {e}")))
    }

    #[wasm_bindgen(js_name = set_frequency)]
    pub fn set_frequency(&mut self, hz: f32) {
        self.core.set_frequency(hz);
    }

    #[wasm_bindgen(js_name = set_amplitude)]
    pub fn set_amplitude(&mut self, amp: f32) {
        self.core.set_amplitude(amp);
    }
}

#[wasm_bindgen]
pub fn abi_version() -> String { "1.0".to_string() }
