use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnimationConfig {
    pub frequency_hz: f32,
    pub amplitude: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AnimationInputs {
    // add inputs as needed (sliders, events, etc.)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnimationOutputs {
    pub value: f32,
}

pub struct AnimationCore {
    cfg: AnimationConfig,
    t: f32,
}

impl AnimationCore {
    pub fn new(cfg: AnimationConfig) -> Self {
        Self { cfg, t: 0.0 }
    }

    pub fn update(&mut self, dt: f32, _inputs: AnimationInputs) -> AnimationOutputs {
        self.t += dt / 50.0;
        let phase = 2.0 * std::f32::consts::PI * self.cfg.frequency_hz * self.t;
        let value = phase.sin() * self.cfg.amplitude;
        AnimationOutputs { value }
    }

    pub fn set_frequency(&mut self, hz: f32) {
        self.cfg.frequency_hz = hz.max(0.0);
    }

    pub fn set_amplitude(&mut self, amp: f32) {
        self.cfg.amplitude = amp;
    }
}
