# Bevy 0.19 draws the Quori face on the API 34/35 arm64 emulator with a host GPU only: software Vulkan loses the device at wgpu-hal's 1 s acquire timeout, and every GLES build fails before its first frame
<!-- tags: family=project; type=reference; project_status=proposal; topics=android,bevy,wgpu,emulator,gfxstream,swiftshader,lavapipe,ci,vizij-rs -->
> **Last updated:** 2026-09-18
> **Related documents:** [Central plan](../bevy_everywhere-plan.md) · [Android packaging spike](bevy-android-spike.md)

Spike directory (scripts, screenshots, logcats, emulator logs, the Vulkan
probe, the animation APK):
`/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/challenge-android/`
(paths below are relative to it; `bevy-android/` is the sibling packaging
spike). Registry sources are quoted as `<crate>-<version>/path:line`. Stack
under test: Bevy 0.19.1, wgpu 29.0.4, emulator 36.6.11 on an Apple M1, arm64
`google_apis` images API 32, 34 and 35, 2560x1600 tablet AVDs (2 vCPUs, 3 GB),
three APKs of `--release` Rust code: the packaging spike's Vulkan build
(`bevy-android/bevy-spike-vulkan-debug.apk`) and `bevy_render/gles` build
(`bevy-android/bevy-spike-gles-debug.apk`), and the animation build
(`bevy-spike-anim-debug.apk`, `app-anim/`: the same app with the spinning
cube scaled to the face's fitted bounds, `app-anim/src/lib.rs:156-161`).

## Findings

1. **The face draws under Vulkan on the API 34 and API 35 images with `-gpu
   host` (MoltenVK through gfxstream), and on nothing else.** Screenshots show
   the Quori face (screen frame, eyes, mouth) over the clear colour:
   `api34-host-vulkan-t30.png` (30 s after launch; `-t25.png` is still the
   clear colour), `api35-host2-vulkan-t15.png`, `api35-host2-vulkan-t45.png`.
   Logcat carries no wgpu error for these runs: adapter `Apple M1`, driver
   `MoltenVK 1.4.0`, "GPU preprocessing is fully supported on this device"
   (`api34-host-vulkan-summary.txt`, `api35-host2-vulkan-logcat.txt`). The
   full matrix is in Evidence.
2. **ANIM-FINDING**
3. **`dumpsys gfxinfo` is not evidence for a NativeActivity.** It counts HWUI
   frames only and reads `Total frames rendered: 0` for every run, including
   the ones that drew (`api34-host-vulkan-gfxinfo.txt`, `api35-host-vulkan-gfxinfo.txt`).
   The screenshot is the frame evidence; the pixel heuristic of `run-case.sh`
   is confounded by the tablet taskbar's launcher icons (45 "orange" cells in
   every API 34/35 screenshot; `api34-swiftshader-vulkan-alone-t45.png`).
4. **SwiftShader Vulkan never presents: wgpu-hal turns a slow frame into a
   device loss.** The surface stays black for 17 s (API 35) to 44 s (API 34,
   emulator alone) after window creation, then logcat reads `Caught DeviceLost
   error: Unknown Unexpected error variant (driver implementation is at
   fault)` and the render system panics in `Buffer::get_mapped_range`
   (`wgpu-29.0.4/src/backend/wgpu_core.rs:2253`;
   `api35-swiftshader-vulkan-logcat.txt` window 16:47:53.766, loss
   16:48:11.204; `api34-swiftshader-vulkan-alone-logcat.txt` 16:36:27.740 →
   16:37:11.220). The mechanism is in wgpu-hal: `Surface::acquire` waits on
   the acquired image's fence with the frame timeout
   (`wgpu-hal-29.0.4/src/vulkan/swapchain/native.rs:473-474`, timeout from
   `wgpu-core-29.0.4/src/present.rs:32,170`, `FRAME_TIMEOUT_MS = 1000`), and a
   `VK_TIMEOUT` there goes through `map_host_device_oom_and_lost_err` →
   `map_host_device_oom_err` → `DeviceError::Unexpected`
   (`wgpu-hal-29.0.4/src/vulkan/mod.rs:1387-1393,1401-1406,1463-1470`); the
   Display of that variant is the logged text (`wgpu-hal-29.0.4/src/lib.rs:381`).
   wgpu issue #9029 (open, February 2026) reports the same mapping on a
   Windows Intel GPU. The host side agrees that frames exceed seconds: the
   emulator logs `Timeout when waiting for the Qsri fence.` then `Destroyed
   VkDevice` (`emulator-api34-swiftshader.log:120-121,144-145`), gfxstream's 3 s
   bound on the present fence
   (`gfxstream-vk_android_native_buffer_operations.cpp:667,837-843`). Not a
   limit problem: the API 34/35 SwiftShader ICD reports `maxBufferSize = 1 GiB`
   (`vkprobe-api34-swiftshader.txt`, `vkprobe-api35-swiftshader.txt`).
   SMALL-FINDING
5. **LAVAPIPE-FINDING**
6. **The `max_buffer_size (0)` failure is the API 32 image's guest Vulkan
   driver, under both GPU modes, and no other image has it.** The probe
   (`vkprobe/src/main.rs`, mirroring
   `wgpu-hal-29.0.4/src/vulkan/adapter.rs:1764-1767,1787-1799`) shows the API 32
   device advertising `apiVersion 1.3.0` without `VK_KHR_maintenance4`, so
   wgpu-hal chains `VkPhysicalDeviceMaintenance4Properties` by API version and
   reads `maxBufferSize = 0` (`vkprobe-api32-swiftshader.txt`); wgpu-hal takes
   `min(maxBufferSize, maxMemoryAllocationSize, i32::MAX)` with no guard
   against zero (`adapter.rs:1427-1447`). API 34/35 report 1 GiB (SwiftShader)
   and 9093 MiB (MoltenVK) (`vkprobe-api35-host.txt`). The emulator release
   notes date Vulkan 1.3 through gfxstream to "system image with API 34"
   (emulator 33.1.23). wgpu's CHANGELOG through v30.0.1 has no entry for
   `max_buffer_size`, `maintenance4` or the emulator: no fixed version exists;
   the API 32 image is what is retired.
7. **The `bevy_render/gles` build fails on every image and GPU mode, for two
   structural reasons that are not emulator quirks.** (a) Where the translator
   reports GLES 3.1 with compute (SwiftShader/ANGLE, all three images),
   `EnvironmentMapGenerationPlugin` queues its cubemap-SPD pipelines at plugin
   `finish` (`bevy_pbr-0.19.1/src/light_probe/generate.rs:103-125,356,372`,
   added by `light_probe/mod.rs:380`); their shader binds `mip_6` as
   `read_write` (`bevy_core_pipeline-0.19.1/src/mip_generation/downsample.wgsl:16,54`,
   `generate.rs:199`), which GLSL ES 3.1 forbids for non-r32 formats ("image
   variables must be qualified readonly and/or writeonly"), and Bevy's default
   error policy exits the app on the internal error
   (`api32-swiftshader-gles-summary.txt`, `api34-swiftshader-gles-summary.txt`,
   `api35-swiftshader-gles-summary.txt`: "Quitting the application due to
   Internal RenderError"). (b) Where the translator reports GLES 3.0 without
   compute (`-gpu host`, Metal GL), the plugin disables itself and the run
   reaches `Surface::configure`, which is refused: `Downlevel flags
   DownlevelFlags(SURFACE_VIEW_FORMATS) are required` then the `Surface is
   not configured for presentation` panic
   (`wgpu-29.0.4/src/backend/wgpu_core.rs:3934`; `api34-host-gles-summary.txt`,
   `api35-host2-gles-summary.txt`, `bevy-android/logcat-launch-gpuhost-gles.txt`).
   wgpu-hal's GLES adapter never sets `SURFACE_VIEW_FORMATS` (the flag is set
   only in `wgpu-hal-29.0.4/src/vulkan/adapter.rs:684-687`; wgpu issue #10337
   proposes it for GLES over `EXT_sRGB_write_control`, open); Bevy asks for an
   sRGB view format whenever the surface's format list has no sRGB entry
   (`bevy_render-0.19.1/src/view/window/mod.rs:394-410`), and wgpu-hal lists
   sRGB GLES surface formats only when EGL is 1.5 or exposes
   `EGL_KHR_gl_colorspace` (`gles/egl.rs:415-424`, `gles/adapter.rs:1248-1259`),
   which the emulator's EGL does not. A hardware GLES 3.1 device hits (a); a
   GLES 3.0 device with `EGL_KHR_gl_colorspace` avoids both.
8. **The same `SURFACE_VIEW_FORMATS` check applies to Vulkan on hardware whose
   swapchain lists no sRGB format.** On Vulkan the flag needs
   `VK_KHR_swapchain_mutable_format` (`wgpu-hal-29.0.4/src/vulkan/adapter.rs:684-687`);
   none of the emulator ICDs have it (`vkprobe-*.txt`), and the host-GPU runs
   pass only because the swapchain offers an sRGB format first. Bevy issue
   #15452 (open) reports the failure on a real Android 12 device (Maleoon 910,
   Vulkan, Bevy 0.14.2); wgpu issue #10340 (September 2026, PR #10357)
   records that Android devices generally lack the extension and argues the
   flag out of the strict-compliance set. This is the one adapter property to
   read on the tablet before anything else.
9. **The emulator's system is starved while the spike runs**: every host-GPU
   run logs ANRs of unrelated processes (`ANR in com.android.phone`, the
   launcher, gms) and shows an "isn't responding" dialog over the face
   (`api35-host2-vulkan-logcat.txt`, 15 ANR lines); the app stays
   `topResumedActivity` and its window survives. ANIM-RATE
10. **Naga's debug `OpSource` breaks gfxstream's guest SPIR-V validator** on
    emulator images ("SPIR-V ERROR: Invalid source language operand: 10",
    water-rs/waterui issue #764, fixed by PR #849 by stripping the debug and
    validation instance flags on emulator guests). Only builds with
    `InstanceFlags::DEBUG` are affected: wgpu-hal attaches naga debug info
    under that flag (`wgpu-hal-29.0.4/src/vulkan/adapter.rs:2624,2639`), and
    `InstanceFlags::from_build_config()` sets it under `debug_assertions`
    (`wgpu-types-29.0.4/src/instance.rs:258-264`), which Bevy's
    `WgpuSettings::default()` starts from (`bevy_render-0.19.1/src/settings.rs:130-144`).
    The spike's `--release` Rust code is not affected; a CI smoke on the
    emulator must use a release-profile library or set `WGPU_DEBUG=0`.
11. **Hardware reports found**: a Bevy 0.19 APK runs on a Pixel 6 Pro and a
    Pixel 11 Pro XL while the same APK loses its device on the emulator
    (AceVik/baylee issue #1, open); the `driver implementation is at fault`
    loss on SwiftShader/gfxstream every 30–35 s under load is reported
    independently (water-rs/map-gpu issue #19, water-rs/waterui issue #860,
    which recreates the wgpu stack after each loss); a Bevy 0.14 app fails on
    a Maleoon 910 with the `SURFACE_VIEW_FORMATS` flag (bevy #15452). No
    report of Bevy 0.19/wgpu 29 failing to render on a mainstream Adreno or
    Mali device was found.
12. **Where a GPU-backed emulator can run in CI.** GitHub's `ubuntu-latest`
    has KVM and no GPU (`-gpu host` there is software rendering). GitHub's
    arm64 macOS runners cannot run the emulator at all: "Nested-virtualization
    is not supported due to the limitation of Apple's Virtualization
    Framework" (GitHub docs, "Supported runners and hardware resources"), and
    the arm64 emulator needs Hypervisor.framework. GitHub's GPU larger runners
    (Linux/Windows, NVIDIA T4, GA since July 2024) are the only hosted option
    with a GPU; whether KVM is available on them is NOT RUN. A self-hosted
    Apple-silicon Mac is the shape verified here. ros-viz-rs's smoke job
    (Bevy 0.18, x86_64 API 34, `-gpu swiftshader_indirect`,
    `ros-viz-rs/.github/workflows/android-apk.yml:101-130`,
    `android/scripts/smoke-test.sh`) asserts that the process is alive and
    grabs a screenshot; it never asserted a drawn frame.

## Questions answered

**Q1 — Per image, GPU mode and backend: did a frame draw?** See the matrix in
Evidence. Drew: API 34 host Vulkan, API 35 host Vulkan, and the animation
build on API 35 host Vulkan (finding 2). Did not: every SwiftShader run
(Vulkan: device lost at the acquire timeout, finding 4; GLES: shader
translation exit, finding 7a), LAVAPIPE-Q1 every GLES run (finding 7),
everything on API 32 (findings 6 and 7). The CI candidate is **arm64 API 35
`google_apis` (API 34 equivalent), `-gpu host`, the Vulkan build,
release-profile Rust code**, on a machine with a GPU (finding 12).

**Q2 — Two screenshots a few seconds apart.** ANIM-Q2

**Q3 — What is known about wgpu 29 / Bevy 0.19 on emulators and hardware.**
Findings 4, 6, 8, 10, 11: the zero buffer limit is the Android 12 image's
driver and has no wgpu fix; the SwiftShader device loss is wgpu-hal's
1 s acquire timeout reported as `DeviceError::Unexpected` (wgpu #9029, open
in 29.0.4 and absent from the CHANGELOG through 30.0.1), compounded by
gfxstream's 3 s present-fence bound, and is reported by three other projects;
the `SURFACE_VIEW_FORMATS` requirement is Bevy's sRGB view format meeting a
driver without `swapchain_mutable_format`, seen on one real device and under
discussion in wgpu (#10340/#10357); naga debug info kills the emulator's
SPIR-V validator in debug builds; real Pixels run Bevy 0.19.

**Q4 — Risk statement for section 5 risk 1 and P0 Gate 0.** Text in
"Implications for the plan".

## Evidence

Matrix (arm64 `google_apis` images; "drew" = face visible in the screenshot;
every row's logcat is `<case>-logcat.txt` or the `bevy-android/` file named):

| Image | `-gpu` | Backend | Result | Evidence |
|---|---|---|---|---|
| API 32 | swiftshader_indirect | Vulkan | no frame: `Buffer size 32 is greater than the maximum buffer size (0)`, panic `wgpu_core.rs:2253` | `bevy-android/logcat-launch.txt`, `bevy-android/screen.png` (launcher) |
| API 32 | host | Vulkan | no frame: same limit-0 error with adapter `Apple M1`/MoltenVK | `bevy-android/logcat-launch-gpuhost-vulkan.txt`, `bevy-android/screen-gpuhost-vulkan.png` (launcher) |
| API 32 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api32-swiftshader-gles-summary.txt`, `api32-swiftshader-gles.png` (launcher) |
| API 32 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused, panic `wgpu_core.rs:3934` | `bevy-android/logcat-launch-gpuhost-gles.txt` |
| API 34 | swiftshader_indirect | Vulkan | no frame: black 44 s, DeviceLost, panic | `api34-swiftshader-vulkan-alone-summary.txt`, `-t5..t45.png` |
| API 34 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api34-swiftshader-gles-summary.txt` |
| API 34 | host | Vulkan | **drew** at t=30 s (t=25 s clear colour only) | `api34-host-vulkan-t25.png`, `-t30.png`, `-summary.txt` |
| API 34 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused | `api34-host-gles-summary.txt` |
| API 35 | swiftshader_indirect | Vulkan | no frame: black, DeviceLost at +17 s, panic | `api35-swiftshader-vulkan-summary.txt`, `api35-swiftshader-vulkan.png` (black) |
| API 35 | swiftshader_indirect | GLES | no frame: GLSL ES image-qualifier error, app exits | `api35-swiftshader-gles-summary.txt`, `api35-swiftshader-gles.png` (launcher) |
| API 35 | host | Vulkan | **drew** at t=15 s | `api35-host2-vulkan-t15.png`, `-t45.png`, `-summary.txt` |
| API 35 | host | GLES | no frame: `SURFACE_VIEW_FORMATS` refused | `api35-host2-gles-summary.txt`, `api35-host2-gles.png` |
| API 35 | host | Vulkan, animation build | ANIM-ROW | |
| API 35 | lavapipe | Vulkan, animation build | LAVAPIPE-ROW | |
| API 35, 640x480 | swiftshader_indirect | Vulkan, animation build | SMALL-ROW | |

Vulkan limits per ICD (`vkprobe-<label>.txt`; `ash` program in `vkprobe/`):

| Image, `-gpu` | Device | `maintenance4` ext | `maxBufferSize` | `maxMemoryAllocationSize` | `swapchain_mutable_format` | wgpu `max_buffer_size` |
|---|---|---|---|---|---|---|
| API 32, swiftshader | SwiftShader Device (LLVM 10.0.0), api 1.3.0 | no | 0 | 1 GiB | no | 0 |
| API 34, swiftshader | same | no | 1 GiB | 1 GiB | no | 1 GiB |
| API 35, swiftshader | same | no | 1 GiB | 1 GiB | no | 1 GiB |
| API 34/35, host | Apple M1 (MoltenVK), api 1.3.0 | no | 9093 MiB | 9093 MiB | no | `i32::MAX` |
LAVAPIPE-PROBE

Runs and timings: `run-case.sh` (10 s wait, one screenshot), `run-case2.sh`
(uninstall first, 20 s, two screenshots), `run-timeline.sh` (screenshot every
5 s with dominant colours), `run-timeline2.sh` (adds the changed-pixel count
of the lower-left quadrant between consecutive screenshots, the cube's
place), `run-live.sh` / `run-host-image.sh` / `run-sw-image.sh` /
`run-anim.sh` (boot, probe, cases, kill). Animation build:
`build-anim-cargo-2.log` (`cargo ndk -t arm64-v8a -P 26 -o
android/app/src/main/jniLibs build --release --lib`, BUILD-TIME), then
`./gradlew --no-daemon assembleDebug` (`build-anim-gradle.log`). Boots on this
Mac: API 35 host 113–149 s, API 34 host 79 s, API 35 swiftshader 43 s, API 34
swiftshader 794 s (with two other emulators running), API 32 swiftshader
258 s. `api35-host-vulkan-manual.png` is a truncated PNG (unusable);
`api35-host-vulkan.png` (first run, three emulators up) shows a black surface
under a launcher ANR dialog.

## Implications for the target picture

- Section 3.6's Android view is renderable with the plan's stack (Bevy 0.19.1,
  wgpu 29, `android-native-activity`, Vulkan): the face draws from the APK's
  own `assets/quori.glb` through `AndroidAssetReader` under an orthographic
  camera fitted to the meshes, on two current emulator images, and ANIM-TP.
- The `bevy_render/gles` fallback build is dead with this Bevy/wgpu pair
  (finding 7): the plan should not carry it. On hardware, Vulkan is the only
  path; the one property that can still refuse it is a swapchain without an
  sRGB format on a driver without `swapchain_mutable_format` (finding 8).
- SW-TP
- Bevy 0.19's `RenderErrorPolicy` default exits the app on internal errors
  and ignores Vulkan validation errors; a `DeviceLost` on the emulator ends
  as a panic inside a system, not a recovery. On a tablet a device loss
  (thermal, driver reset) needs `RenderErrorPolicy::Recover` or an explicit
  restart — a P6 item, not a gate.

## Implications for the plan

**Section 5, risk 1 (replacement text):** Android rendering on Bevy 0.19 is
verified on the emulator with a host GPU: the Quori face draws under Vulkan on
the arm64 API 34 and API 35 `google_apis` images with `-gpu host` (MoltenVK
through gfxstream), and ANIM-RISK (S:challenge-android-render 1–2). It draws
under no GLES build (two structural failures in Bevy 0.19.1 + wgpu 29; the
fallback build is dropped). SW-RISK The API 32 image is the only one with the
zero buffer limit and is retired. What remains for the tablet: (1) the
adapter's swapchain formats and `VK_KHR_swapchain_mutable_format` — a
swapchain without an sRGB format fails `Surface::configure` exactly as GLES
does (bevy #15452 on one real device); (2) frame rate and morph animation on
hardware, which the emulator cannot measure (ANIM-RATE-SHORT); (3) the
device-loss recovery policy. The fallback (Tauri shell around the wasm view)
stays recorded but is not on the path.

**P0 Gate 0 (replacement text):** Passed on the emulator with: arm64 API 35
`google_apis` (API 34 equivalent), `-gpu host`, Vulkan build, release-profile
Rust code; evidence = a screenshot at 30 s showing the face, two screenshots
5 s apart differing where the animated content is, no `Caught` line in
logcat; `dumpsys gfxinfo` is dropped from the evidence list. The CI smoke
image is that combination on a machine with a GPU: a self-hosted
Apple-silicon Mac runner (the verified shape), or a GitHub GPU larger runner
if KVM is available there (NOT RUN); GitHub's arm64 macOS runners cannot run
the emulator (no nested virtualization). SW-GATE Debug-profile Rust builds are
excluded from the emulator smoke (naga debug info kills gfxstream's
validator). The tablet run of P6's gate adds the swapchain-format probe
(`vkprobe` extended with a surface, or `vulkaninfo`) as its first step.

**Section 3.9:** the APK job builds `--release` Rust code for the smoke; the
smoke script asserts the screenshot and the screenshot-to-screenshot change
of the animated region, and samples `dumpsys meminfo`; the GLES feature and
its build are removed from the matrix.

## Open questions

- Frame rate and animation on hardware: ANIM-OPEN; nothing here bounds a
  tablet.
- Whether the tablet's Vulkan swapchain lists an sRGB format or supports
  `VK_KHR_swapchain_mutable_format` (finding 8); if neither, Bevy 0.19.1 fails
  to configure the surface and the fix is in `bevy_render` (render to a
  non-sRGB surface with manual encoding) or in wgpu (#10357).
- Whether GitHub's GPU larger runners expose KVM to the emulator (NOT RUN).
- Whether Bevy's `RenderErrorPolicy::Recover` brings the render world back
  after a device loss on Android (NOT RUN; the SwiftShader loss ends in a
  system panic under the default policy).
- Mesh picking, `bevy_egui` panels and the SAF/VIEW entry (packaging spike
  finding 11) are untested on the images that draw.
