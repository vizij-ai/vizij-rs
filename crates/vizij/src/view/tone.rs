//! Tone mapping, as the web renderer's three.js applies it: a curve from the
//! linear color a material shades to the linear color written out, per
//! fragment, before blending — and never to the background.
//!
//! The view's materials are unlit with their shading composed on the CPU
//! ([`Surface`](super::Surface)), so a face's tone mapping is composed there
//! too, into each material's color: for a flat-colored face that is exactly
//! what three's per-fragment curve computes, and it is set per face, where a
//! camera's post-process would be bound to whatever the camera draws. The
//! curves are three's (r170, `tonemapping_pars_fragment`) at an exposure of 1,
//! so a face authored under one reads the same here.

// The constants are three's source digits, kept verbatim so they read
// against it; f32 rounds them as the GPU does.
#![allow(clippy::excessive_precision)]

/// A tone-mapping curve, by the names the authoring app stores in a bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToneMapping {
    /// The color as shaded, clipped at 1 by the output: three's
    /// `NoToneMapping`, the web renderer's default.
    #[default]
    None,
    /// three's `AgXToneMapping`: Blender's view transform, after Filament.
    Agx,
    /// three's `ACESFilmicToneMapping`, with its 1/0.6 exposure lift.
    Aces,
    /// three's `NeutralToneMapping`: Khronos PBR Neutral.
    Neutral,
}

impl ToneMapping {
    /// The curve a name stands for: `none`, `agx`, `aces` or `neutral`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "agx" => Some(Self::Agx),
            "aces" => Some(Self::Aces),
            "neutral" => Some(Self::Neutral),
            _ => None,
        }
    }

    /// Map a linear RGB color.
    pub fn apply(self, rgb: [f32; 3]) -> [f32; 3] {
        match self {
            Self::None => rgb,
            Self::Agx => agx(rgb),
            Self::Aces => aces(rgb),
            Self::Neutral => neutral(rgb),
        }
    }
}

/// `m · v` for a matrix given as its columns, as GLSL's `mat3(c0, c1, c2)`
/// is: the constants below are copied column for column from three's source.
fn mul(columns: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    let [c0, c1, c2] = columns;
    [0, 1, 2].map(|i| c0[i] * v[0] + c1[i] * v[1] + c2[i] * v[2])
}

fn aces(rgb: [f32; 3]) -> [f32; 3] {
    const INPUT: [[f32; 3]; 3] = [
        [0.59719, 0.07600, 0.02840],
        [0.35458, 0.90834, 0.13383],
        [0.04823, 0.01566, 0.83777],
    ];
    const OUTPUT: [[f32; 3]; 3] = [
        [1.60475, -0.10208, -0.00327],
        [-0.53108, 1.10813, -0.07276],
        [-0.07367, -0.00605, 1.07602],
    ];
    let lifted = rgb.map(|c| c / 0.6);
    let fitted = mul(INPUT, lifted).map(|v| {
        let a = v * (v + 0.0245786) - 0.000090537;
        let b = v * (0.983729 * v + 0.4329510) + 0.238081;
        a / b
    });
    mul(OUTPUT, fitted).map(|c| c.clamp(0.0, 1.0))
}

fn agx(rgb: [f32; 3]) -> [f32; 3] {
    const SRGB_TO_REC2020: [[f32; 3]; 3] = [
        [0.6274, 0.0691, 0.0164],
        [0.3293, 0.9195, 0.0880],
        [0.0433, 0.0113, 0.8956],
    ];
    const REC2020_TO_SRGB: [[f32; 3]; 3] = [
        [1.6605, -0.1246, -0.0182],
        [-0.5876, 1.1329, -0.1006],
        [-0.0728, -0.0083, 1.1187],
    ];
    const INSET: [[f32; 3]; 3] = [
        [0.856627153315983, 0.137318972929847, 0.11189821299995],
        [0.0951212405381588, 0.761241990602591, 0.0767994186031903],
        [0.0482516061458583, 0.101439036467562, 0.811302368396859],
    ];
    const OUTSET: [[f32; 3]; 3] = [
        [
            1.1271005818144368,
            -0.1413297634984383,
            -0.14132976349843826,
        ],
        [
            -0.11060664309660323,
            1.157823702216272,
            -0.11060664309660294,
        ],
        [
            -0.016493938717834573,
            -0.016493938717834257,
            1.2519364065950405,
        ],
    ];
    const MIN_EV: f32 = -12.47393;
    const MAX_EV: f32 = 4.026069;
    let encoded = mul(INSET, mul(SRGB_TO_REC2020, rgb)).map(|c| {
        let x = ((c.max(1e-10).log2() - MIN_EV) / (MAX_EV - MIN_EV)).clamp(0.0, 1.0);
        // The default-contrast sigmoid, as a polynomial.
        let x2 = x * x;
        let x4 = x2 * x2;
        15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
            - 0.00232
    });
    let linear = mul(OUTSET, encoded).map(|c| c.max(0.0).powf(2.2));
    mul(REC2020_TO_SRGB, linear).map(|c| c.clamp(0.0, 1.0))
}

fn neutral(rgb: [f32; 3]) -> [f32; 3] {
    const START_COMPRESSION: f32 = 0.8 - 0.04;
    const DESATURATION: f32 = 0.15;
    let x = rgb[0].min(rgb[1]).min(rgb[2]);
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let color = rgb.map(|c| c - offset);
    let peak = color[0].max(color[1]).max(color[2]);
    if peak < START_COMPRESSION {
        return color;
    }
    let d = 1.0 - START_COMPRESSION;
    let new_peak = 1.0 - d * d / (peak + d - START_COMPRESSION);
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    color.map(|c| {
        let c = c * new_peak / peak;
        c + (new_peak - c) * g
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
    }

    #[test]
    fn none_is_the_identity() {
        let rgb = [0.2, 1.7, 0.0];
        assert_eq!(ToneMapping::None.apply(rgb), rgb);
    }

    #[test]
    fn names_are_the_authoring_apps() {
        for (name, curve) in [
            ("none", ToneMapping::None),
            ("agx", ToneMapping::Agx),
            ("aces", ToneMapping::Aces),
            ("neutral", ToneMapping::Neutral),
        ] {
            assert_eq!(ToneMapping::from_name(name), Some(curve));
        }
        assert_eq!(ToneMapping::from_name("filmic"), None);
    }

    /// Values computed by three r170's shader functions at exposure 1.
    #[test]
    fn the_curves_are_threes() {
        let grey = [0.18, 0.18, 0.18];
        // ACES lifts mid grey (exposure / 0.6) and keeps it grey.
        let aces = ToneMapping::Aces.apply(grey);
        assert!(close(aces, [aces[0]; 3]), "{aces:?}");
        assert!((aces[0] - 0.2131).abs() < 1e-3, "{aces:?}");
        // Neutral below its compression knee only subtracts the toe offset.
        assert!(close(
            ToneMapping::Neutral.apply([0.5, 0.3, 0.1]),
            [0.46, 0.26, 0.06]
        ));
        // AgX maps black to its floor and white below 1.
        let black = ToneMapping::Agx.apply([0.0; 3]);
        assert!(black.iter().all(|c| *c < 1e-3), "{black:?}");
        let white = ToneMapping::Agx.apply([1.0; 3]);
        assert!(white.iter().all(|c| *c > 0.5 && *c < 1.0), "{white:?}");
    }

    /// Every curve but none keeps an over-bright color in range, and keeps
    /// the order of two greys.
    #[test]
    fn the_curves_compress_into_range_monotonically() {
        for curve in [ToneMapping::Agx, ToneMapping::Aces, ToneMapping::Neutral] {
            let bright = curve.apply([4.0, 2.0, 8.0]);
            assert!(bright.iter().all(|c| (0.0..=1.0).contains(c)), "{curve:?}");
            let mut last = -1.0;
            for step in 0..=20 {
                let v = curve.apply([step as f32 / 10.0; 3])[1];
                assert!(v >= last, "{curve:?} at {step}");
                last = v;
            }
        }
    }
}
