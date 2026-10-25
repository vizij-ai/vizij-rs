//! The settings pane over the face, on desktop and Android: the Vizij
//! standalone's Settings page, drawn with egui, and the device's operator.
//!
//! A tap (or click) on the face shows the Settings button, at the bottom of
//! the screen, and the device's name, at the top, when it is registered with
//! Semio Studio; another tap hides them. The button opens the pane, which
//! the Close button, the Android back gesture and Escape close. The pane
//! holds:
//!
//! - the face shown, by its id, and **Open a face…**, which hands the
//!   platform an [`OpenFace`] message (the file dialog on desktop, the
//!   document picker on Android);
//! - the **background**, kept per face in `settings.json` in the app's data
//!   directory ([`store`]): opening a face again brings back the background
//!   chosen for it, and a face with none kept shows the app's;
//! - the **device**: where it stands with Semio Studio, and what it
//!   registers as — its name, model family and owners — in a form. Saving
//!   the form keeps the identity and registers it: an associated device is
//!   registered anew, the device restarting on its face with the Studio
//!   bridge; one not associated yet associates under it. The owners are
//!   Studio user ids, each an owner: arora 15 registers no other role
//!   ([`RuntimeHandle::studio`]);
//! - the device's **questions**: the pane is its operator ([`PaneOperator`]).
//!   A question the registration asks, or a client's access request, opens
//!   the pane on it; an access request counts down to being allowed.
//!
//! A back gesture with the pane closed is the platform's ([`LeaveApp`]).
//! Edge-hugging controls keep clear of the [`SafeArea`] the platform
//! reports.

mod operator;
mod store;

pub use operator::PaneOperator;

use std::path::PathBuf;
use std::time::Instant;

use arora_bridge::{AccessDecision, DeviceInfo};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::CameraOutputMode;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy::window::PrimaryWindow;
use bevy_egui::{
    egui, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext,
};

use crate::native::{Identity, RuntimeHandle, StudioStatus};
use crate::view::meta::FaceMeta;
use crate::view::{Face, Superseded, ViewOptions};

/// The pane, over the face a platform's app shows.
pub struct SettingsPlugin {
    /// The running device.
    pub handle: RuntimeHandle,
    /// The device's operator, which the pane draws the questions of: the
    /// one the device was started with ([`crate::native::Mode::OperatedBy`]).
    pub operator: PaneOperator,
    /// The file the settings are kept in.
    pub file: PathBuf,
}

/// Open a face: the platform shows its file picker, and the face picked
/// goes to the device ([`RuntimeHandle::reload`]).
#[derive(Message)]
pub struct OpenFace;

/// A back gesture the pane did not take: the platform leaves the app.
#[derive(Message)]
pub struct LeaveApp;

/// The margins, in logical pixels, that the screen's edges keep from the
/// pane and its controls: the system bars and the display's cutout, which
/// the platform reports.
#[derive(Resource, Clone, Copy, Default, Debug, PartialEq)]
pub struct SafeArea {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            // The pane draws through a camera of its own, above the faces'.
            .insert_resource(EguiGlobalSettings {
                auto_create_primary_context: false,
                ..default()
            })
            .init_resource::<SafeArea>()
            .add_message::<OpenFace>()
            .add_message::<LeaveApp>()
            .insert_resource(Pane {
                handle: self.handle.clone(),
                operator: self.operator.clone(),
                kept: store::Kept::load(self.file.clone()),
                open: false,
                controls: false,
                face: None,
                app_background: None,
                form: Form::default(),
                press: None,
            })
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (follow_face, back))
            .add_systems(EguiPrimaryContextPass, (style, draw).chain());
    }
}

/// The pane's camera orders above every face's ([`crate::view::CLEAR_ORDER`]
/// and the slots above it).
const PANE_ORDER: isize = 1_000;

/// How long a press may last and how far it may move, in logical pixels,
/// to be a tap.
const TAP_SECS: f32 = 0.4;
const TAP_SLOP: f32 = 16.0;

/// The standalone's colours: the Settings button (Tailwind's blue-600) and
/// the pane (neutral-900 at 95 %).
const BUTTON: egui::Color32 = egui::Color32::from_rgb(37, 99, 235);
const PANE_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(22, 22, 22, 242);

/// The backgrounds offered at a tap: black, the standalone's grey, white,
/// the web page's, then four colours. The custom colour button gives any.
const SWATCHES: [[u8; 3]; 8] = [
    [0x00, 0x00, 0x00],
    [0x73, 0x73, 0x73],
    [0xff, 0xff, 0xff],
    [0x10, 0x11, 0x14],
    [0x1e, 0x3a, 0x8a],
    [0x0f, 0x76, 0x6e],
    [0x9d, 0x17, 0x4d],
    [0xb4, 0x53, 0x09],
];

#[derive(Resource)]
struct Pane {
    handle: RuntimeHandle,
    operator: PaneOperator,
    kept: store::Kept,
    open: bool,
    /// The Settings button (and the device's name) shown over the face.
    controls: bool,
    /// The id of the face shown.
    face: Option<String>,
    /// The background of a face with none kept: the app's, as it launched.
    app_background: Option<Color>,
    form: Form,
    press: Option<Press>,
}

/// The device form: what the device registers with Studio as, as the
/// person edits it.
#[derive(Default)]
struct Form {
    name: String,
    model_family: String,
    /// One owner's Studio user id per row.
    owners: Vec<String>,
    /// What the fields were filled from: what the device registers as when
    /// the pane opened or its standing with Studio last changed. `None`
    /// fills them again.
    filled_from: Option<DeviceInfo>,
    /// The identity the form sent, until it is edited again or the pane
    /// closes; nothing is sent again meanwhile.
    sent: Option<Identity>,
}

impl Form {
    /// Fill the fields from `info`, unless they already were: what the
    /// person typed stays while the device stands where it stood.
    fn fill(&mut self, info: &DeviceInfo) {
        if self.filled_from.as_ref() == Some(info) {
            return;
        }
        self.name = info.name.clone().unwrap_or_default();
        self.model_family = info.model_family.clone().unwrap_or_default();
        self.owners = info.owners.clone();
        // A row to type the first owner in.
        if self.owners.is_empty() {
            self.owners.push(String::new());
        }
        self.filled_from = Some(info.clone());
    }

    /// The owners the rows give: trimmed, blanks and repeats dropped.
    fn owners(&self) -> Vec<String> {
        let mut owners: Vec<String> = Vec::new();
        for owner in self.owners.iter().map(|owner| owner.trim()) {
            if !owner.is_empty() && !owners.iter().any(|kept| kept == owner) {
                owners.push(owner.to_string());
            }
        }
        owners
    }

    fn model_family(&self) -> Option<String> {
        Some(self.model_family.trim())
            .filter(|family| !family.is_empty())
            .map(str::to_string)
    }

    /// Whether the fields say other than `info` registers.
    fn differs_from(&self, info: &DeviceInfo) -> bool {
        self.name.trim() != info.name.as_deref().unwrap_or_default().trim()
            || self.model_family() != info.model_family
            || self.owners() != info.owners
    }

    /// The identity the fields give, a blank name generated.
    fn identity(&self) -> Identity {
        Identity {
            name: Some(self.name.trim())
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .unwrap_or_else(generated_name),
            owners: self.owners(),
            model_family: self.model_family(),
        }
    }
}

/// A press in progress: where and when it began, and whether it began on
/// the pane.
struct Press {
    at: Vec2,
    since: f32,
    on_pane: bool,
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: PANE_ORDER,
            clear_color: ClearColorConfig::None,
            // Over what the faces drew, as their cameras draw over the clear.
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::Default,
            },
            ..default()
        },
        // It draws the pane alone.
        RenderLayers::none(),
        Tonemapping::None,
        DebandDither::Disabled,
        // The faces' cameras' sampling: one target, one sample count.
        Msaa::Sample4,
        PrimaryEguiContext,
    ));
}

/// The id a face's background is kept under.
fn face_key(meta: &FaceMeta) -> String {
    meta.bundle
        .face_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .unwrap_or("face")
        .to_string()
}

/// A face shown shows its background: the one kept for it, else the app's.
fn follow_face(
    faces: Query<&Face, (Added<Face>, Without<Superseded>)>,
    mut pane: ResMut<Pane>,
    mut options: ResMut<ViewOptions>,
) {
    let app_background = *pane.app_background.get_or_insert(options.background);
    for face in &faces {
        let key = face_key(&face.meta);
        options.background = pane
            .kept
            .background(&key)
            .map(|[r, g, b]| Color::srgb_u8(r, g, b))
            .unwrap_or(app_background);
        pane.face = Some(key);
    }
}

/// The Android back gesture (and Escape) closes the pane; with the pane
/// closed, the back gesture is the platform's.
fn back(
    mut keys: MessageReader<KeyboardInput>,
    mut pane: ResMut<Pane>,
    mut leave: MessageWriter<LeaveApp>,
) {
    for key in keys.read() {
        if !key.state.is_pressed() || key.repeat {
            continue;
        }
        match (&key.logical_key, pane.open) {
            (Key::BrowserBack | Key::Escape, true) => pane.open = false,
            (Key::BrowserBack, false) => {
                leave.write(LeaveApp);
            }
            _ => {}
        }
    }
}

/// The pane's look: egui's dark theme at touch sizes.
fn style(mut contexts: EguiContexts, mut done: Local<bool>) -> Result {
    if *done {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        use egui::{FontId, TextStyle};
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.interact_size.y = 32.0;
        for (text, size) in [
            (TextStyle::Body, 16.0),
            (TextStyle::Button, 16.0),
            (TextStyle::Small, 13.0),
            (TextStyle::Heading, 22.0),
            (TextStyle::Monospace, 15.0),
        ] {
            let family = style
                .text_styles
                .get(&text)
                .map(|font| font.family.clone())
                .unwrap_or_default();
            style.text_styles.insert(text, FontId::new(size, family));
        }
    });
    *done = true;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut contexts: EguiContexts,
    mut pane: ResMut<Pane>,
    mut options: ResMut<ViewOptions>,
    safe: Res<SafeArea>,
    touches: Res<Touches>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut open_face: MessageWriter<OpenFace>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if std::mem::take(&mut pane.operator.asks().raised) {
        pane.open = true;
    }
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    tap(
        &mut pane,
        &ctx,
        &touches,
        &mouse,
        cursor,
        time.elapsed_secs(),
    );
    if pane.open {
        draw_pane(&ctx, &mut pane, &mut options, &safe, &mut open_face);
    } else {
        // A form left unsaved is dropped: the pane opens on what the device
        // registers as.
        pane.form.filled_from = None;
        pane.form.sent = None;
        if pane.controls {
            draw_controls(&ctx, &mut pane, &safe);
        }
    }
    Ok(())
}

/// A tap that the pane does not take toggles the controls.
fn tap(
    pane: &mut Pane,
    ctx: &egui::Context,
    touches: &Touches,
    mouse: &ButtonInput<MouseButton>,
    cursor: Option<Vec2>,
    now: f32,
) {
    let pressed = touches
        .iter_just_pressed()
        .next()
        .map(|touch| touch.position())
        .or_else(|| {
            mouse
                .just_pressed(MouseButton::Left)
                .then_some(cursor)
                .flatten()
        });
    if let Some(at) = pressed {
        pane.press = Some(Press {
            at,
            since: now,
            on_pane: ctx.is_pointer_over_egui() || ctx.egui_wants_pointer_input(),
        });
        return;
    }
    let released = touches
        .iter_just_released()
        .next()
        .map(|touch| touch.position())
        .or_else(|| {
            mouse
                .just_released(MouseButton::Left)
                .then_some(cursor)
                .flatten()
        });
    if let (Some(at), Some(press)) = (released, pane.press.take()) {
        let tapped = now - press.since <= TAP_SECS && at.distance(press.at) <= TAP_SLOP;
        if tapped && !press.on_pane && !pane.open {
            pane.controls = !pane.controls;
        }
    }
}

/// The Settings button over the face, and the device's name once Studio
/// knows it.
fn draw_controls(ctx: &egui::Context, pane: &mut Pane, safe: &SafeArea) {
    egui::Area::new(egui::Id::new("settings-button"))
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0.0, -(16.0 + safe.bottom)),
        )
        .show(ctx, |ui| {
            let button =
                egui::Button::new(egui::RichText::new("Settings").color(egui::Color32::WHITE))
                    .fill(BUTTON)
                    .corner_radius(24.0)
                    .min_size(egui::vec2(120.0, 44.0));
            if ui.add(button).clicked() {
                pane.open = true;
            }
        });
    if let StudioStatus::Connected(info) = pane.handle.studio() {
        if let Some(name) = info.name.filter(|name| !name.is_empty()) {
            egui::Area::new(egui::Id::new("device-name"))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 8.0 + safe.top))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(egui::Color32::from_black_alpha(128))
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(name).color(egui::Color32::WHITE));
                        });
                });
        }
    }
}

fn draw_pane(
    ctx: &egui::Context,
    pane: &mut Pane,
    options: &mut ViewOptions,
    safe: &SafeArea,
    open_face: &mut MessageWriter<OpenFace>,
) {
    let screen = ctx.viewport_rect();
    let margin = |inset: f32| (inset + 16.0).clamp(0.0, 127.0) as i8;
    let margins = egui::Margin {
        left: margin(safe.left),
        right: margin(safe.right),
        top: margin(safe.top),
        bottom: margin(safe.bottom),
    };
    egui::Area::new(egui::Id::new("settings"))
        .fixed_pos(screen.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANE_FILL)
                .inner_margin(margins)
                .show(ui, |ui| {
                    let inner = screen.size() - margins.sum();
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    ui.horizontal(|ui| {
                        ui.heading("Settings");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                pane.open = false;
                            }
                        });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        questions(ui, &pane.operator);
                        face_section(ui, pane, open_face);
                        background_section(ui, pane, options);
                        device_section(ui, pane);
                    });
                });
        });
}

/// A section's title, small and dim as the standalone's.
fn title(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .small()
            .color(egui::Color32::from_gray(160)),
    );
}

fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).color(egui::Color32::WHITE)).fill(BUTTON)
}

/// The device's questions, first: what the pane opened for.
fn questions(ui: &mut egui::Ui, operator: &PaneOperator) {
    let mut asks = operator.asks();
    if let Some(ask) = asks.texts.front_mut() {
        title(ui, "Semio Studio asks");
        ui.label(&ask.label);
        let field = ui.add(
            egui::TextEdit::singleline(&mut ask.draft)
                .desired_width(f32::INFINITY)
                .char_limit(512),
        );
        let entered = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let answerable = !ask.required || !ask.draft.trim().is_empty();
        let mut answer = None;
        ui.horizontal(|ui| {
            if ui.add_enabled(answerable, primary("Answer")).clicked() || (entered && answerable) {
                answer = Some(Some(ask.draft.clone()));
            }
            if !ask.required && ui.button("Skip").clicked() {
                answer = Some(None);
            }
        });
        if let Some(answer) = answer {
            if let Some(ask) = asks.texts.pop_front() {
                ask.answer(answer);
            }
        }
    }
    if let Some(ask) = asks.access.front() {
        title(ui, "Access request");
        ui.label(ask.request.describe());
        let left = ask
            .until
            .saturating_duration_since(Instant::now())
            .as_secs_f32()
            .ceil();
        ui.label(
            egui::RichText::new(format!("Allowed in {left:.0} s unless you decide"))
                .color(egui::Color32::from_gray(160)),
        );
        let mut ruling = None;
        ui.horizontal_wrapped(|ui| {
            for (label, decision, remember) in [
                ("Allow", AccessDecision::Allowed, false),
                ("Reject", AccessDecision::Rejected, false),
                ("Always allow", AccessDecision::Allowed, true),
                ("Always reject", AccessDecision::Rejected, true),
            ] {
                if ui.button(label).clicked() {
                    ruling = Some((decision, remember));
                }
            }
        });
        if let Some((decision, remember)) = ruling {
            if let Some(ask) = asks.access.pop_front() {
                ask.rule(decision, remember);
            }
        }
    }
}

fn face_section(ui: &mut egui::Ui, pane: &mut Pane, open_face: &mut MessageWriter<OpenFace>) {
    title(ui, "Current face");
    ui.label(pane.face.as_deref().unwrap_or("Unknown"));
    if ui.add(primary("Open a face…")).clicked() {
        open_face.write(OpenFace);
        pane.open = false;
    }
}

fn background_section(ui: &mut egui::Ui, pane: &mut Pane, options: &mut ViewOptions) {
    title(ui, "Background");
    let mut rgb = options.background.to_srgba().to_u8_array_no_alpha();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for swatch in SWATCHES {
            let [r, g, b] = swatch;
            let selected = rgb == swatch;
            let button = egui::Button::new("")
                .fill(egui::Color32::from_rgb(r, g, b))
                .min_size(egui::vec2(40.0, 40.0))
                .corner_radius(6.0)
                .stroke(if selected {
                    egui::Stroke::new(3.0, egui::Color32::WHITE)
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_gray(90))
                });
            if ui.add(button).clicked() {
                rgb = swatch;
                changed = true;
            }
        }
        changed |= egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed();
    });
    if changed {
        let [r, g, b] = rgb;
        options.background = Color::srgb_u8(r, g, b);
        let key = pane.face.clone().unwrap_or_else(|| "face".to_string());
        pane.kept.set_background(&key, rgb);
    }
}

fn dim(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).color(egui::Color32::from_gray(170))
}

/// Where the device stands with Studio, and what it registers as.
fn device_section(ui: &mut egui::Ui, pane: &mut Pane) {
    let status = pane.handle.studio();
    if status == StudioStatus::Off {
        return;
    }
    title(ui, "Device");
    const ASSOCIATE: &str = "Associate with Semio Studio";
    let sent = match status {
        StudioStatus::Off => None,
        StudioStatus::Connecting => {
            ui.label("Connecting to Semio Studio…");
            None
        }
        StudioStatus::Connected(info) => {
            ui.label("Registered with Semio Studio");
            if let Some(software) = info.software_version.as_deref().filter(|v| !v.is_empty()) {
                ui.label(dim(format!("Software {software}")));
            }
            pane.form.fill(&info);
            // Studio's record after the registration, against what was sent:
            // an update leaves a field the identity leaves out as it was.
            match &pane.form.sent {
                Some(sent) if holds(&info, sent) => {
                    ui.label(
                        egui::RichText::new("Saved — registered with Semio Studio.")
                            .color(egui::Color32::from_rgb(74, 222, 128)),
                    );
                }
                Some(_) => {
                    ui.label(dim(
                        "Saved. Studio keeps the device as shown: a model family cleared \
                         here stays in Studio.",
                    ));
                }
                None => {}
            }
            device_form(ui, &mut pane.form, &info, "Save device info", true)
        }
        StudioStatus::Idle(kept) => {
            ui.label("Not associated with Semio Studio.");
            let info = kept.unwrap_or_default();
            pane.form.fill(&info);
            device_form(ui, &mut pane.form, &info, ASSOCIATE, false)
        }
        StudioStatus::Declined => {
            ui.label("Studio declined: no owner was given.");
            let info = DeviceInfo::default();
            pane.form.fill(&info);
            device_form(ui, &mut pane.form, &info, ASSOCIATE, false)
        }
        StudioStatus::Failed(reason) => {
            ui.label(format!("The Studio connection failed: {reason}"));
            if ui.add(primary("Try again")).clicked() {
                pane.handle.associate();
            }
            None
        }
    };
    if let Some(identity) = sent {
        pane.handle.register(identity);
    }
}

/// Whether Studio's record `info` holds the identity `sent`.
fn holds(info: &DeviceInfo, sent: &Identity) -> bool {
    info.name.as_deref() == Some(sent.name.as_str())
        && info.owners == sent.owners
        && info.model_family == sent.model_family
}

/// The device's name, model family and owners, filled from `info`, which
/// the device registers as or, not associated, would. Returns the identity
/// to register once `action` is pressed: with an owner at least, and for a
/// `registered` device only when it changes.
fn device_form(
    ui: &mut egui::Ui,
    form: &mut Form,
    info: &DeviceInfo,
    action: &str,
    registered: bool,
) -> Option<Identity> {
    let mut edited = false;
    ui.label(dim("Device name"));
    edited |= ui
        .add(
            egui::TextEdit::singleline(&mut form.name)
                .hint_text("Blank for a generated one")
                .desired_width(f32::INFINITY),
        )
        .changed();
    ui.label(dim("Model family"));
    edited |= ui
        .add(
            egui::TextEdit::singleline(&mut form.model_family)
                .hint_text("Optional")
                .desired_width(f32::INFINITY),
        )
        .changed();
    ui.label(dim("Owners"));
    ui.label(
        dim(
            "Each owner's Semio Studio user ID (Firebase UID): the device appears in their \
             Studio account.",
        )
        .small(),
    );
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let mut removed = None;
    for (index, owner) in form.owners.iter_mut().enumerate() {
        ui.allocate_ui_with_layout(
            row,
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui.button("Remove").clicked() {
                    removed = Some(index);
                }
                edited |= ui
                    .add(
                        egui::TextEdit::singleline(owner)
                            .hint_text("Studio user ID")
                            .desired_width(ui.available_width()),
                    )
                    .changed();
            },
        );
    }
    if let Some(index) = removed {
        form.owners.remove(index);
        edited = true;
    }
    if ui.button("Add owner").clicked() {
        form.owners.push(String::new());
    }
    if edited {
        form.sent = None;
    }
    let owned = !form.owners().is_empty();
    if !owned {
        ui.label(dim("A device registers under one owner at least."));
    }
    let ready = owned && form.sent.is_none() && (!registered || form.differs_from(info));
    if ui.add_enabled(ready, primary(action)).clicked() {
        let identity = form.identity();
        form.sent = Some(identity.clone());
        return Some(identity);
    }
    None
}

/// A `vizij-<random>` device name, as the standalone generated.
fn generated_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default(),
    );
    let mut value = hasher.finish();
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let suffix: String = (0..6)
        .map(|_| {
            let c = ALPHABET[(value % ALPHABET.len() as u64) as usize] as char;
            value /= ALPHABET.len() as u64;
            c
        })
        .collect();
    format!("vizij-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered() -> DeviceInfo {
        DeviceInfo {
            name: Some("vizij-bench".into()),
            model_family: Some("vizij".into()),
            owners: vec!["uid-1".into(), "uid-2".into()],
            ..DeviceInfo::default()
        }
    }

    /// The form opens on what the device registers as, and keeps what the
    /// person types until the device stands elsewhere.
    #[test]
    fn the_form_holds_the_registration_and_the_person_s_edits() {
        let mut form = Form::default();
        form.fill(&registered());
        assert_eq!(form.name, "vizij-bench");
        assert_eq!(form.model_family, "vizij");
        assert_eq!(form.owners, ["uid-1", "uid-2"]);
        assert!(!form.differs_from(&registered()));

        form.name = "vizij-renamed".into();
        form.owners.push("uid-3".into());
        form.fill(&registered());
        assert_eq!(form.name, "vizij-renamed", "an edit stays");
        assert!(form.differs_from(&registered()));
        assert_eq!(
            form.identity(),
            Identity {
                name: "vizij-renamed".into(),
                owners: vec!["uid-1".into(), "uid-2".into(), "uid-3".into()],
                model_family: Some("vizij".into()),
            }
        );

        // Registered anew as edited: the form follows.
        let renamed = DeviceInfo {
            name: Some("vizij-renamed".into()),
            ..registered()
        };
        form.fill(&renamed);
        assert_eq!(form.owners, ["uid-1", "uid-2"]);
        assert!(!form.differs_from(&renamed));
    }

    /// Each row is one owner: blanks and repeats are dropped, and a model
    /// family emptied gives none.
    #[test]
    fn the_rows_give_each_owner_once() {
        let mut form = Form::default();
        form.fill(&registered());
        form.owners = vec![
            " uid-1 ".into(),
            String::new(),
            "uid-2".into(),
            "uid-1".into(),
        ];
        form.model_family = "  ".into();
        assert_eq!(form.owners(), ["uid-1", "uid-2"]);
        assert_eq!(form.identity().model_family, None);
        assert!(form.differs_from(&registered()), "the model family emptied");
    }

    /// The record Studio holds after a registration holds what was sent
    /// only when every field the identity gives is there: Studio keeps a
    /// model family the identity leaves out.
    #[test]
    fn studio_s_record_holds_what_was_sent_field_by_field() {
        let sent = Identity {
            name: "vizij-bench".into(),
            owners: vec!["uid-1".into(), "uid-2".into()],
            model_family: Some("vizij".into()),
        };
        assert!(holds(&registered(), &sent));
        let cleared = Identity {
            model_family: None,
            ..sent.clone()
        };
        assert!(!holds(&registered(), &cleared), "Studio kept the family");
        let reordered = Identity {
            owners: vec!["uid-2".into(), "uid-1".into()],
            ..sent
        };
        assert!(!holds(&registered(), &reordered));
    }

    /// A device not associated opens on a row for its first owner, and a
    /// name left blank is generated as it registers.
    #[test]
    fn a_device_not_associated_opens_blank_and_gets_a_generated_name() {
        let mut form = Form::default();
        form.fill(&DeviceInfo::default());
        assert_eq!(form.owners, [""]);
        assert!(form.owners().is_empty());
        form.owners[0] = "uid-1".into();
        let identity = form.identity();
        assert!(identity.name.starts_with("vizij-"), "{}", identity.name);
        assert_eq!(identity.owners, ["uid-1"]);
    }
}
