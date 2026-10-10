---
"@vizij/animation": major
---

A number track's values come out in the width of its keypoints. Keypoints given as `{"f64": …}` output `{"f64": …}` from updates and bakes, derivatives included, where they output `{"f32": …}`; a track with any `{"f64": …}` keypoint outputs `{"f64": …}` throughout, and a key an `f32` and an `f64` track blend into outputs `{"f64": …}` on the steps the `f64` track weighs on it. Keypoints given as `{"f32": …}`, and stored-animation JSON numbers, still output `{"f32": …}`.
