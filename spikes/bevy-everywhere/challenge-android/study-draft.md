# Bevy 0.19 draws the Quori face on the API 34/35 arm64 emulator with a host GPU; every software-rendered and every GLES combination fails before the first frame
<!-- tags: family=project; type=reference; project_status=proposal; topics=android,bevy,wgpu,emulator,gfxstream,swiftshader,ci,vizij-rs -->
> **Last updated:** 2026-09-18
> **Related documents:** [Central plan](../bevy_everywhere-plan.md) · [Android packaging spike](bevy-android-spike.md)

Spike directory (scripts, APKs, screenshots, logcats, emulator logs, Vulkan
probe): `/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/challenge-android/`
(paths below are relative to it; `bevy-android/` is the sibling packaging spike).
Registry sources are quoted as `<crate>-<version>/path:line`. Stack under test:
Bevy 0.19.1, wgpu 29.0.4, the two spike APKs of the packaging spike (Vulkan
build `bevy-spike-vulkan-debug.apk`, `bevy_render/gles` build
`bevy-spike-gles-debug.apk`, both `--release` Rust code), emulator 36.6.11 on an
Apple M1, arm64 `google_apis` images API 32, 34, 35, 2560x1600 tablet AVDs.

## Findings

1. **The face draws under Vulkan on the API 34 and API 35 images with `-gpu host`
   (MoltenVK through gfxstream), and on nothing else.** Screenshots show the
   Quori face (eyes, mouth, screen frame) over the clear colour:
   `api34-host-vulkan-t30.png` (30 s after launch), `api35-host2-vulkan-t15.png`
   (15 s), `api35-host2-vulkan-t45.png`. Logcat carries no wgpu error for these
   runs (`api34-host-vulkan-summary.txt`, `api35-host2-vulkan-summary.txt`:
   adapter `Apple M1`, driver `MoltenVK 1.4.0`, "GPU preprocessing is fully
   supported"). The full matrix is in Evidence.
2. **`dumpsys gfxinfo` is not evidence for a NativeActivity.** It counts HWUI
   frames only; it reads `Total frames rendered: 0` for every run including the
   two that drew (`api34-host-vulkan-summary.txt`, `api35-host2-gles-summary.txt`).
   The screenshot and `dumpsys SurfaceFlinger --latency <layer>` are the
   frame evidence; the pixel heuristic of `run-case.sh` is confounded by the
   tablet taskbar's launcher icons (45 "orange" cells in every API 34/35
   screenshot, `api34-swiftshader-vulkan-alone-t45.png` shows why).
3. **SwiftShader Vulkan never presents: the guest's device is lost before the
   first frame, on API 34 and API 35 alike.** Surface stays (0,0,0) for 45 s
   (`api34-swiftshader-vulkan-alone-summary.txt`, nine screenshots), then
   `Caught DeviceLost error: Unknown Unexpected error variant (driver
   implementation is at fault)` and the `Buffer::get_mapped_range` panic
   (`wgpu-29.0.4/src/backend/wgpu_core.rs:2253`); 44 s after window creation
   alone, 15–18 s with another emulator running
   (`api34-swiftshader-vulkan-summary.txt`, `api35-swiftshader-vulkan-summary.txt`).
   The emulator side logs `Timeout when waiting for the Qsri fence.` then
   `Destroyed VkDevice` each time (`emulator-api34-swiftshader.log:120-121,144-145`,
   `emulator-api35-swiftshader.log`). gfxstream bounds its wait on the
   present (QueueSignalReleaseImageANDROID) fence to 3 s
   (`gfxstream-vk_android_native_buffer_operations.cpp:667,837-843`, fetched
   from google/gfxstream); SwiftShader does not finish Bevy's first frames at
   2560x1600 within it. Not a wgpu limit: the API 34/35 SwiftShader ICD reports
   `maxBufferSize = 1 GiB` (`vkprobe-api34-swiftshader.txt`,
   `vkprobe-api35-swiftshader.txt`).
4. **The `max_buffer_size (0)` failure is the API 32 image's guest Vulkan
   driver, under both GPU modes, and no other image has it.** The probe
   (`vkprobe/src/main.rs`, mirroring
   `wgpu-hal-29.0.4/src/vulkan/adapter.rs:1764-1767,1787-1799`) shows the API 32
   device advertising `apiVersion 1.3.0` without `VK_KHR_maintenance4`, so
   wgpu-hal chains `VkPhysicalDeviceMaintenance4Properties` by API version and
   reads `maxBufferSize = 0` (`vkprobe-api32-swiftshader.txt`); wgpu-hal takes
   `min(maxBufferSize, maxMemoryAllocationSize, i32::MAX)` with no guard against
   zero (`adapter.rs:1427-1447`). API 34/35 report 1 GiB (SwiftShader) and
   9093 MiB (MoltenVK) (`vkprobe-api35-host.txt`). The emulator release notes
   date Vulkan 1.3 through gfxstream to "system image with API 34" (emulator
   33.1.23). wgpu's CHANGELOG through v28 has no entry for `max_buffer_size`,
   `maintenance4` or the emulator: no fixed version exists; the API 32 image is
   what is retired.
5. **The `bevy_render/gles` build fails on every image and GPU mode, for two
   structural reasons that are not emulator quirks.** (a) Where the translator
   reports GLES 3.1 with compute (SwiftShader/ANGLE, all three images),
   `EnvironmentMapGenerationPlugin` queues its cubemap-SPD pipelines at plugin
   `finish` (`bevy_pbr-0.19.1/src/light_probe/generate.rs:103-125,356,372`,
   added by `light_probe/mod.rs:380`); their shader binds `mip_6` as
   `read_write` (`bevy_core_pipeline-0.19.1/src/mip_generation/downsample.wgsl:16,54`,
   `generate.rs:199`), which GLSL ES 3.1 forbids for non-r32 formats
   ("image variables must be qualified readonly and/or writeonly"), and Bevy's
   default error policy exits the app on the internal error
   (`api32-swiftshader-gles-summary.txt`, `api34-swiftshader-gles-summary.txt`,
   `api35-swiftshader-gles-summary.txt`: "Quitting the application due to
   Internal RenderError"). (b) Where the translator reports GLES 3.0 without
   compute (`-gpu host`, Metal GL), the plugin disables itself and the run
   reaches `Surface::configure`, which is refused: `Downlevel flags
   DownlevelFlags(SURFACE_VIEW_FORMATS) are required` then the
   `Surface is not configured for presentation` panic
   (`wgpu-29.0.4/src/backend/wgpu_core.rs:3934`; `api34-host-gles-summary.txt`,
   `api35-host2-gles-summary.txt`, `bevy-android/logcat-launch-gpuhost-gles.txt`).
   wgpu-hal's GLES adapter never sets `SURFACE_VIEW_FORMATS`
   (`wgpu-hal-29.0.4/src/gles/adapter.rs:390-447`); Bevy asks for an sRGB view
   format whenever the surface's format list has no sRGB entry
   (`bevy_render-0.19.1/src/view/window/mod.rs:394-410`), and wgpu-hal lists
   sRGB GLES surface formats only when EGL is 1.5 or exposes
   `EGL_KHR_gl_colorspace` (`gles/egl.rs:415-424`, `gles/adapter.rs:1248-1259`),
   which the emulator's EGL does not. A hardware GLES 3.1 device would hit (a);
   a GLES 3.0 device with EGL_KHR_gl_colorspace would avoid both.
6. **The same `SURFACE_VIEW_FORMATS` check applies to Vulkan on hardware whose
   swapchain lists no sRGB format.** On Vulkan the flag needs
   `VK_KHR_swapchain_mutable_format` (`wgpu-hal-29.0.4/src/vulkan/adapter.rs:684-687`);
   none of the emulator ICDs have it (`vkprobe-*.txt`), and the host-GPU runs
   pass only because the swapchain offers an sRGB format first. Bevy issue
   #15452 reports the failure on a real Android 12 device (Maleoon 910, Vulkan,
   Bevy 0.14.2). This is the one adapter property to read on the tablet before
   anything else.
7. **Software Vulkan alternatives: PENDING-LAVAPIPE and PENDING-SMALL.**
8. **Animation: PENDING-ANIM.**
9. **The emulator's system is starved while the spike runs**: every host-GPU
   run logs ANRs of unrelated processes (`ANR in com.android.phone`, the
   launcher, gms) and shows an "isn't responding" dialog over the face
   (`api35-host2-vulkan-logcat.txt`, 15 ANR lines); the app itself stays
   `topResumedActivity` and its window survives. The AVDs have 2 vCPUs and
   3 GB; the host was also running a cargo build during `api35-host2`. The
   `fit: bounds` log line (emitted after 180 `Update` frames) never appears
   within 45–60 s in any run: the app runs far below 60 Hz on the emulator.
10. **Naga's debug `OpSource` breaks gfxstream's guest SPIR-V validator** on
    emulator images ("SPIR-V ERROR: Invalid source language operand: 10",
    water-rs/waterui PR #849): only builds with `InstanceFlags::DEBUG` (cargo
    debug profile) are affected; the spike's `--release` Rust code is not. A CI
    smoke run on the emulator must use a release-profile library or strip the
    flag.
11. **Hardware reports found**: a Bevy 0.19 APK runs on a Pixel 6 Pro and a
    Pixel 11 Pro XL while the same APK loses its device on the emulator
    (AceVik/baylee issue #1); the `driver implementation is at fault` device
    loss on SwiftShader/gfxstream every 30–35 s is reported independently
    (water-rs/map-gpu #19, water-rs/waterui #860); a Bevy 0.14 app fails on a
    Maleoon 910 with the `SURFACE_VIEW_FORMATS` flag (bevy #15452). No report
    of Bevy 0.19/wgpu 29 failing to render on a mainstream Adreno or Mali
    device was found.

## Questions answered

**Q1 — Per image, GPU mode and backend: did a frame draw?** See the matrix in
Evidence. Drew: API 34 host Vulkan, API 35 host Vulkan. Did not: every
SwiftShader run (Vulkan: device lost, finding 3; GLES: shader translation
exit, finding 5a), every GLES run (finding 5), everything on API 32 (finding 4
and 5). The CI candidate is **API 35 `google_apis` arm64 (or API 34), `-gpu
host`, the Vulkan build, release-profile Rust code**, on a runner with a GPU
(finding 12 in Evidence for what that means for GitHub-hosted runners).

**Q2 — Two screenshots a few seconds apart.** PENDING-ANIM-ANSWER.

**Q3 — What is known about wgpu 29 / Bevy 0.19 on emulators and hardware.**
Findings 4, 6, 10, 11: the zero buffer limit is the Android 12 emulator image
and has no wgpu fix; SwiftShader device loss is gfxstream's 3 s present-fence
bound and is reported by three other projects; the `SURFACE_VIEW_FORMATS`
requirement is Bevy's sRGB view format meeting a driver without
`swapchain_mutable_format`, seen on one real device; naga debug info kills
the emulator's SPIR-V validator in debug builds; real Pixels run Bevy 0.19.

**Q4 — Risk statement for section 5 risk 1 and P0 Gate 0.** Text in
"Implications for the plan".

## Evidence

Matrix (arm64 `google_apis` images; "drew" = face visible in the screenshot;
every row's logcat is `<case>-logcat.txt` or the `bevy-android/` file named):

| Image | `-gpu` | Backend | Result | Evidence |
|---|---|---|---|---|
| API 32 | swiftshader_indirect | Vulkan | no frame: `Buffer size 32 is greater than the maximum buffer size (0)`, panic `wgpu_core.rs:2253` | `bevy-android/logcat-launch.txt`, `bevy-android/screen.png` (launcher) |
| API 32 | host | Vulkan | no frame: same limit-0 error with adapter `Apple M1`/MoltenVK | `bevy-android/logcat-launch-gpuhost-vulkan.txt` |
| API 32 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api32-swiftshader-gles-summary.txt`, `api32-swiftshader-gles.png` (launcher) |
| API 32 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused, panic `wgpu_core.rs:3934` | `bevy-android/logcat-launch-gpuhost-gles.txt` |
| API 34 | swiftshader_indirect | Vulkan | no frame: black 45 s, DeviceLost at +44 s, panic | `api34-swiftshader-vulkan-alone-summary.txt`, `-t5..t45.png` |
| API 34 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api34-swiftshader-gles-summary.txt` |
| API 34 | host | Vulkan | **drew** at t=30 s (t=25 s clear colour only) | `api34-host-vulkan-t25.png`, `-t30.png`, `-summary.txt` |
| API 34 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused | `api34-host-gles-summary.txt` |
| API 35 | swiftshader_indirect | Vulkan | no frame: black, DeviceLost at +18 s, panic | `api35-swiftshader-vulkan-summary.txt`, `api35-swiftshader-vulkan.png` |
| API 35 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api35-swiftshader-gles-summary.txt`, `api35-swiftshader-gles.png` (launcher) |
| API 35 | host | Vulkan | **drew** at t=15 s; frame content changes between t=15 s and t=45 s | `api35-host2-vulkan-t15.png`, `-t45.png`, `-summary.txt` |
| API 35 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused | `api35-host2-gles-summary.txt`, `api35-host2-gles-logcat.txt` |
| API 35 | lavapipe | Vulkan | PENDING-LAVAPIPE-ROW | |
| API 35 640x480 | swiftshader_indirect | Vulkan | PENDING-SMALL-ROW | |

Vulkan limits per ICD (`vkprobe-<label>.txt`; `ash` program in `vkprobe/`):

| Image, `-gpu` | Device | `maintenance4` ext | `maxBufferSize` | `maxMemoryAllocationSize` | `swapchain_mutable_format` | wgpu `max_buffer_size` |
|---|---|---|---|---|---|---|
| API 32, swiftshader | SwiftShader Device (LLVM 10.0.0), api 1.3.0 | no | 0 | 1 GiB | no | 0 |
| API 34, swiftshader | same | no | 1 GiB | 1 GiB | no | 1 GiB |
| API 35, swiftshader | same | no | 1 GiB | 1 GiB | no | 1 GiB |
| API 34/35, host | Apple M1 (MoltenVK), api 1.3.0 | no | 9093 MiB | 9093 MiB | no | `i32::MAX` |

Runs and timings: `run-case.sh` (10 s wait, one screenshot), `run-case2.sh`
(uninstall first, 20 s, two screenshots), `run-timeline.sh` (screenshot every
5 s with dominant colours), `run-live.sh` / `run-host-image.sh` /
`run-sw-image.sh` / `run-anim.sh` (boot, probe, cases, kill). Boots on this
Mac: API 35 host 113–149 s, API 34 host 79 s, API 35 swiftshader 43 s, API 34
swiftshader 794 s (with two other emulators running), API 32 swiftshader
258 s. Screenshot `api35-host-vulkan-manual.png` is a truncated PNG (unusable);
`api35-host-vulkan.png` (first run, three emulators up) shows a black surface
under a launcher ANR dialog with app-area pixels (10,11,13), i.e. the clear
colour dimmed, consistent with the later runs.

12. **What "a runner with a GPU" means.** GitHub's `ubuntu-latest` has KVM and
    no GPU: `-gpu host` there is software rendering, i.e. finding 3. GitHub's
    Apple-silicon macOS runners have a GPU and Hypervisor.framework; running
    the arm64 image with `-gpu host` there is the shape verified here but was
    NOT RUN on a hosted runner. ros-viz-rs's smoke job (Bevy 0.18, x86_64 API
    34, `-gpu swiftshader_indirect`, `ros-viz-rs/.github/workflows/android-apk.yml:101-130`)
    asserts only that the process is alive and grabs a screenshot; it never
    asserted a drawn frame.

## Implications for the target picture

- Section 3.6's Android view is renderable with the plan's stack (Bevy 0.19.1,
  wgpu 29, `android-native-activity`, Vulkan): the face draws from the APK's
  own `assets/quori.glb` through `AndroidAssetReader` under an orthographic
  camera fitted to the meshes, on two current emulator images.
- The `bevy_render/gles` fallback build is dead with this Bevy/wgpu pair
  (finding 5): the plan should not carry it. On hardware, Vulkan is the only
  path; the one property that can still refuse it is a swapchain without an
  sRGB format on a driver without `swapchain_mutable_format` (finding 6).
- Software rendering is not a rendering test bed for this stack (finding 3
  and the pending rows): CI rendering assertions need a GPU-backed emulator.
- Bevy 0.19's `RenderErrorPolicy` default exits the app on internal errors and
  ignores Vulkan validation errors; a `DeviceLost` on the emulator currently
  ends as a panic inside a system, not a recovery. On a tablet a device loss
  (thermal, driver reset) needs `RenderErrorPolicy::Recover` or an explicit
  restart — a P6 item, not a gate.

## Implications for the plan

**Section 5, risk 1 (replacement text):** Android rendering on Bevy 0.19 is
verified on the emulator only with a host GPU: the Quori face draws under
Vulkan on the arm64 API 34 and API 35 `google_apis` images with `-gpu host`
(MoltenVK through gfxstream) (S:challenge-android-render 1). It draws under
no software renderer (SwiftShader loses the device before the first frame,
gfxstream's 3 s present-fence bound; PENDING-SW-SENTENCE) and under no GLES
build (two structural failures in Bevy 0.19.1 + wgpu 29; the fallback build is
dropped). The API 32 image is the only one with the zero buffer limit and is
retired. What remains for the tablet: (1) the adapter's swapchain formats and
`VK_KHR_swapchain_mutable_format` — a swapchain without an sRGB format fails
`Surface::configure` exactly as GLES does (bevy #15452 on one real device);
(2) frame rate and morph animation, which the emulator cannot measure (it runs
below 4 Hz here); (3) device-loss recovery policy. The fallback (Tauri shell
around the wasm view) stays recorded but is not on the path.

**P0 Gate 0 (replacement text):** Pass on the emulator: arm64 API 35
`google_apis` (API 34 equivalent), `-gpu host`, Vulkan build, release-profile
Rust code, screenshot at 30 s shows the face, `dumpsys SurfaceFlinger
--latency <layer>` shows frame timestamps advancing, no `Caught` line in
logcat. The CI smoke image is that combination on a GPU-backed runner
(GitHub macOS arm64 or a self-hosted Mac; NOT RUN on a hosted runner); on
`ubuntu-latest` the smoke can only assert "process alive and no wgpu error
line before the device loss", never a frame. `dumpsys gfxinfo` is dropped
from the evidence list. Debug-profile Rust builds are excluded from the
emulator smoke (naga debug info kills gfxstream's validator). The tablet
run of P6's gate adds the swapchain-format probe (`vkprobe` extended with a
surface, or `vulkaninfo`) as its first step.

**Section 3.9:** the APK job builds `--release` Rust code for the smoke; the
smoke script asserts the screenshot and the SurfaceFlinger timestamps, and
samples `dumpsys meminfo`; the GLES feature and its build are removed from
the matrix.

## Open questions

- Frame rate and animation on hardware: the emulator stays under 4 Hz with
  the release-profile spike library; nothing here bounds a tablet.
- Whether the tablet's Vulkan swapchain lists an sRGB format or supports
  `VK_KHR_swapchain_mutable_format` (finding 6); if neither, Bevy 0.19.1 fails
  to configure the surface and the fix is in `bevy_render` (render to a
  non-sRGB surface with manual encoding) or in wgpu.
- Whether GitHub's macOS arm64 hosted runners boot this AVD with `-gpu host`
  in acceptable time (NOT RUN).
- Whether Bevy's `RenderErrorPolicy::Recover` brings the render world back
  after a device loss on Android (NOT RUN; the SwiftShader loss ends in a
  system panic under the default policy).
- Mesh picking, `bevy_egui` panels and the SAF/VIEW entry (packaging spike
  finding 11) are untested on the images that draw.
