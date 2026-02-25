//! Pure Rust 2D noise algorithms: value noise, Perlin gradient noise, and simplex noise.

/// Deterministic integer hash function for noise generation.
fn hash2d(ix: i32, iy: i32, seed: i32) -> u32 {
    let mut h = (ix as u32)
        .wrapping_mul(374761393)
        .wrapping_add((iy as u32).wrapping_mul(668265263))
        .wrapping_add((seed as u32).wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1103515245);
    h ^ (h >> 16)
}

/// Map hash to float in [0, 1).
fn hash_to_unit(h: u32) -> f32 {
    (h & 0x00FF_FFFF) as f32 / 16777216.0
}

/// Smooth interpolation (Hermite/smoothstep).
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Quintic interpolation for Perlin noise (6t^5 - 15t^4 + 10t^3).
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// 2D gradient from hash (one of 8 unit directions).
fn grad2d(hash: u32, dx: f32, dy: f32) -> f32 {
    match hash & 7 {
        0 => dx + dy,
        1 => -dx + dy,
        2 => dx - dy,
        3 => -dx - dy,
        4 => dx,
        5 => -dx,
        6 => dy,
        _ => -dy,
    }
}

/// 2D value noise: hash-based with bilinear interpolation.
/// Returns a value in [-1, 1].
pub fn value_noise_2d(x: f32, y: f32, seed: i32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    let v00 = hash_to_unit(hash2d(ix, iy, seed));
    let v10 = hash_to_unit(hash2d(ix + 1, iy, seed));
    let v01 = hash_to_unit(hash2d(ix, iy + 1, seed));
    let v11 = hash_to_unit(hash2d(ix + 1, iy + 1, seed));

    let sx = smoothstep(fx);
    let sy = smoothstep(fy);

    let result = lerp(lerp(v00, v10, sx), lerp(v01, v11, sx), sy);
    result * 2.0 - 1.0
}

/// Classic 2D Perlin gradient noise. Returns a value in approximately [-1, 1].
pub fn perlin_noise_2d(x: f32, y: f32, seed: i32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    let u = fade(fx);
    let v = fade(fy);

    let n00 = grad2d(hash2d(ix, iy, seed), fx, fy);
    let n10 = grad2d(hash2d(ix + 1, iy, seed), fx - 1.0, fy);
    let n01 = grad2d(hash2d(ix, iy + 1, seed), fx, fy - 1.0);
    let n11 = grad2d(hash2d(ix + 1, iy + 1, seed), fx - 1.0, fy - 1.0);

    let nx0 = lerp(n00, n10, u);
    let nx1 = lerp(n01, n11, u);
    lerp(nx0, nx1, v)
}

// Simplex noise constants
const F2: f32 = 0.5 * (1.732_050_8 - 1.0); // (sqrt(3) - 1) / 2
const G2: f32 = (3.0 - 1.732_050_8) / 6.0; // (3 - sqrt(3)) / 6

/// 2D simplex noise. Returns a value in approximately [-1, 1].
pub fn simplex_noise_2d(x: f32, y: f32, seed: i32) -> f32 {
    let s = (x + y) * F2;
    let i = (x + s).floor() as i32;
    let j = (y + s).floor() as i32;

    let t = (i + j) as f32 * G2;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as f32 + G2;
    let y1 = y0 - j1 as f32 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let mut n0 = 0.0f32;
    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 >= 0.0 {
        let t0 = t0 * t0;
        n0 = t0 * t0 * grad2d(hash2d(i, j, seed), x0, y0);
    }

    let mut n1 = 0.0f32;
    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 >= 0.0 {
        let t1 = t1 * t1;
        n1 = t1 * t1 * grad2d(hash2d(i + i1, j + j1, seed), x1, y1);
    }

    let mut n2 = 0.0f32;
    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 >= 0.0 {
        let t2 = t2 * t2;
        n2 = t2 * t2 * grad2d(hash2d(i + 1, j + 1, seed), x2, y2);
    }

    // Scale to [-1, 1] range
    70.0 * (n0 + n1 + n2)
}

/// Fractal Brownian motion wrapper for any base noise function.
pub fn fbm<F>(
    x: f32,
    y: f32,
    seed: i32,
    frequency: f32,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    base_noise: F,
) -> f32
where
    F: Fn(f32, f32, i32) -> f32,
{
    let mut sum = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut freq = frequency;
    let mut max_amplitude = 0.0f32;

    for i in 0..octaves {
        let octave_seed = seed.wrapping_add(i as i32 * 12345);
        sum += base_noise(x * freq, y * freq, octave_seed) * amplitude;
        max_amplitude += amplitude;
        freq *= lacunarity;
        amplitude *= persistence;
    }

    if max_amplitude > 0.0 {
        sum / max_amplitude
    } else {
        0.0
    }
}
