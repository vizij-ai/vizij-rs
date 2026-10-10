---
"@vizij/animation-module": major
---

The module (2.0.0) outputs a number track's values in the width of its keypoints. A track keyed with `Value::F64` returns `F64` from `step`, `step_values`, `bake` and `bake_with_derivatives` (derivatives included), sampled and blended in `f64`, where it returned `F32`; a track keyed with `Value::F32` still returns `F32`. A track with any `F64` keypoint returns `F64` throughout; a key an `F32` and an `F64` track blend into returns `F64` on the steps the `F64` track weighs on it. A consumer that writes the values onto typed keys, such as a device's declared inputs, keys every track of a key in the type the key declares. Composites, `ArrayF32` and an `ArrayValue` of numbers stay single precision.
