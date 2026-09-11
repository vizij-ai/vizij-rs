#![cfg(target_arch = "wasm32")]
//! The Vizij renderer mounted on a page.
//!
//! What the page provides: a canvas, a GLB, and values. What it gets back is
//! the same renderer the native app runs — `vizij-render` — with a values
//! feed the page writes into.
//!
//! The GLB arrives twice by design. Bevy's asset server loads it by path to
//! get meshes, materials and morph targets; the same bytes are handed in
//! directly so [`vizij_render::FaceMeta`] can read the `RobotData`
//! extension, which Bevy's loader does not surface. The browser serves the
//! second read from cache.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::camera::SubCameraView;
use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use vizij_api_core::value::{float, vec3};
use vizij_api_core::{TypedPath, Value};
use vizij_render::{
    Anchor, AnchorSink, AnimatableInfo, Face, FaceMeta, Fit, PickSink, PoseFeed, ViewOffsetFeed,
    ViewOptions, ViewPlugin,
};
use wasm_bindgen::prelude::*;

type Feed = Arc<Mutex<Vec<(TypedPath, Value)>>>;

/// The page's handle on the values the renderer reads. One per page: the Bevy
/// app owns the browser's animation loop, so a second view has nothing to run
/// on.
static FEED: OnceLock<Feed> = OnceLock::new();

/// The animatables the loaded face declares, so a page can offer them.
static ANIMATABLES: OnceLock<Vec<AnimatableInfo>> = OnceLock::new();

/// The page's view offset, read once per frame. Plain data, so the render
/// systems can hold it while the page writes it from an animation callback.
static OFFSET: OnceLock<Arc<Mutex<Option<SubCameraView>>>> = OnceLock::new();

fn offset_cell() -> &'static Arc<Mutex<Option<SubCameraView>>> {
    OFFSET.get_or_init(|| Arc::new(Mutex::new(None)))
}

// The page's callbacks. A `js_sys::Function` is not `Send`, and a Bevy
// resource must be, so the sinks the renderer holds are closures that capture
// nothing and look the function up here. Sound because wasm is single
// threaded: the sink runs on the same thread that set it.
thread_local! {
    static ON_PICK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static ON_ANCHORS: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

/// Mounts the renderer on `canvas` (a CSS selector) and starts rendering.
///
/// `glb_path` is resolved by Bevy's asset server under `asset_root`, which is
/// relative to the page's origin — a host serving the renderer from a route
/// rather than a dedicated page needs to say where its assets live.
/// `glb_bytes` must be that same file.
#[wasm_bindgen]
pub fn start(
    canvas: String,
    asset_root: String,
    glb_path: String,
    glb_bytes: &[u8],
) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    let meta =
        FaceMeta::from_glb_bytes(glb_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    log::info!(
        "face read: {} elements, {} animatables",
        meta.elements.len(),
        meta.animatables.len()
    );

    let _ = ANIMATABLES.set(meta.animatable_info());

    let feed: Feed = Arc::new(Mutex::new(Vec::new()));
    if FEED.set(feed.clone()).is_err() {
        return Err(JsValue::from_str("a view is already running on this page"));
    }
    let read = feed.clone();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some(canvas),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root,
                    // Never probe for `.meta` sidecars. Bevy's default is to
                    // ask for one per asset and fall back when the answer is
                    // 404 — but a single-page app answers any unknown path
                    // with its `index.html`, 200 OK, so the fallback never
                    // triggers and the asset fails to load on HTML that will
                    // not parse as RON. Hosts serve this renderer from a
                    // route, so assume the SPA case.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                // The host owns logging. A page has installed the `log` global
                // logger before the renderer starts, so LogPlugin's attempt to
                // install its own fails and reports an error. Disabling it
                // leaves one logger; the `tracing` dependency's `log` feature
                // carries Bevy's own events into it.
                .build()
                .disable::<bevy::log::LogPlugin>(),
        )
        .insert_resource(Face { meta, glb_path })
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: Fit::Contain,
            // Neutral: the fit decides the extent, unmagnified. The native
            // app's `--zoom` default is the same value.
            zoom: Vec2::ONE,
            // The web renderer's ambient intensity.
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        .insert_resource(PoseFeed::new(move || {
            read.lock().map(|values| values.clone()).unwrap_or_default()
        }))
        .insert_resource(PickSink::new(report_pick))
        .insert_resource(AnchorSink::new(report_anchors))
        .insert_resource(ViewOffsetFeed::new(|| {
            offset_cell().lock().ok().and_then(|cell| *cell)
        }))
        .add_plugins(ViewPlugin)
        .add_systems(Update, report_first_frame)
        .run();

    Ok(())
}

/// Logs how long the first frame took, measured from module start — the number
/// a page's cold start is judged on.
fn report_first_frame(mut reported: Local<bool>) {
    if *reported {
        return;
    }
    *reported = true;
    let now = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_default();
    log::info!("first frame at {now:.0} ms since navigation");
}

fn report_pick(element: &str) {
    ON_PICK.with(|cb| {
        if let Some(cb) = cb.borrow().as_ref() {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from_str(element));
        }
    });
}

fn report_anchors(anchors: &[Anchor]) {
    ON_ANCHORS.with(|cb| {
        let Some(cb) = cb.borrow().clone() else {
            return;
        };
        if let Ok(value) = serde_wasm_bindgen::to_value(anchors) {
            let _ = cb.call1(&JsValue::NULL, &value);
        }
    });
}

/// Called with the element name each time a pointer clicks the face.
#[wasm_bindgen]
pub fn set_pick_callback(callback: js_sys::Function) {
    ON_PICK.with(|cb| *cb.borrow_mut() = Some(callback));
}

/// Called once per frame with every element's screen-space box, in logical
/// pixels of the canvas: `{element, x, y, min_x, min_y, max_x, max_y}`.
#[wasm_bindgen]
pub fn set_anchor_callback(callback: js_sys::Function) {
    ON_ANCHORS.with(|cb| *cb.borrow_mut() = Some(callback));
}

/// Renders a sub-rectangle of a larger notional viewport — the analogue of
/// three's `setViewOffset`, for keeping the face centred in the part of the
/// canvas a panel is not covering.
#[wasm_bindgen]
pub fn set_view_offset(
    offset_x: f32,
    offset_y: f32,
    width: u32,
    height: u32,
    full_width: u32,
    full_height: u32,
) {
    if let Ok(mut cell) = offset_cell().lock() {
        *cell = Some(SubCameraView {
            full_size: UVec2::new(full_width, full_height),
            offset: Vec2::new(offset_x, offset_y),
            size: UVec2::new(width, height),
        });
    }
}

/// Renders the whole viewport again.
#[wasm_bindgen]
pub fn clear_view_offset() {
    if let Ok(mut cell) = offset_cell().lock() {
        *cell = None;
    }
}

/// Every animatable the loaded face declares, as
/// `{id, node, feature, morph_target}`, ordered by element then feature.
#[wasm_bindgen]
pub fn animatables() -> Result<JsValue, JsValue> {
    let infos = ANIMATABLES.get().map(Vec::as_slice).unwrap_or_default();
    serde_wasm_bindgen::to_value(infos).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Sets one animatable's scalar value, addressed by its animatable UUID.
///
/// Values replace rather than accumulate: the renderer reads whatever the feed
/// holds at the start of a frame.
#[wasm_bindgen]
pub fn set_float(path: String, value: f32) -> Result<(), JsValue> {
    write(path, float(value))
}

/// Sets one animatable's vector value — a translation, rotation (radians,
/// three.js ZYX order) or scale.
#[wasm_bindgen]
pub fn set_vec3(path: String, x: f32, y: f32, z: f32) -> Result<(), JsValue> {
    write(path, vec3([x, y, z]))
}

fn write(path: String, value: Value) -> Result<(), JsValue> {
    let feed = FEED
        .get()
        .ok_or_else(|| JsValue::from_str("no view is running; call start() first"))?;
    let path = TypedPath::parse(&path).map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
    let mut values = feed
        .lock()
        .map_err(|_| JsValue::from_str("the values feed is poisoned"))?;
    match values.iter_mut().find(|(existing, _)| *existing == path) {
        Some((_, slot)) => *slot = value,
        None => values.push((path, value)),
    }
    Ok(())
}
