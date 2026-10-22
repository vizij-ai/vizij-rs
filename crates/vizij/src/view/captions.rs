//! Captions over the face: the face standard's caption
//! ([`standard::CAPTION_TEXT`], said by [`standard::CAPTION_SPEAKER`]) drawn
//! at the bottom of each face's own rectangle.
//!
//! The view reads the caption keys from the face's rig every frame and plays
//! each new caption through [`vizij_arora_host::captions`] — eight-word
//! chunks, each held long enough to read, queued behind the caption
//! showing, in the speaker's colour. A face's overlay is UI text targeting
//! that face's camera, so it is drawn wherever the face is, window or
//! offscreen target, and is in every frame the view publishes.
//!
//! The text is sized and placed on the face's rectangle as the incumbent
//! ROS4HRI face places it on its 800×600 window: a line of 1/15 of the height
//! (40 px), centred in a band 1/5 of the height (120 px) whose bottom stands
//! 1/30 of the height (20 px) above the bottom edge, 9/10 of the width wide,
//! with a black drop shadow. The font is Inter, embedded, subset to the Latin
//! scripts.

use std::collections::HashMap;

use arora_shape::Value;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use vizij_arora_host::captions::{CaptionQueue, Chunk};
use vizij_arora_host::standard;

use super::{Face, Superseded};

/// Inter 4.001 Regular, subset to Basic Latin, Latin-1, Latin Extended-A/B
/// and the common punctuation; its licence (the SIL Open Font License, no
/// reserved name) is beside it and in its own name records.
const FONT: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular-Latin.ttf");

/// The caption line's height, in percent of the face's rectangle height.
const LINE_VH: f32 = 100.0 / 15.0;
/// The caption band's height and its bottom margin, in percent of the face's
/// rectangle height.
const BAND_VH: f32 = 20.0;
const MARGIN_VH: f32 = 100.0 / 30.0;
/// The drop shadow's offset, logical pixels.
const SHADOW_OFFSET: f32 = 2.0;

pub struct CaptionsPlugin;

impl Plugin for CaptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptionQueues>()
            .add_systems(Startup, load_font)
            .add_systems(
                Update,
                (read_captions, keep_overlays, show_captions)
                    .chain()
                    .after(super::apply_poses),
            );
    }
}

/// Each face's captions over time, by face id: a reload keeps the face's
/// place in them.
#[derive(Resource, Default)]
pub struct CaptionQueues(pub HashMap<String, CaptionQueue>);

/// The embedded font, once loaded into the font assets.
#[derive(Resource)]
struct CaptionFont(Handle<Font>);

/// A face's caption overlay: the band on the face's camera.
#[derive(Component)]
struct CaptionOverlay {
    face: Entity,
}

/// The caption line inside an overlay.
#[derive(Component)]
struct CaptionLine;

fn load_font(mut fonts: ResMut<Assets<Font>>, mut commands: Commands) {
    let font = fonts.add(Font::from_bytes(FONT.to_vec()));
    commands.insert_resource(CaptionFont(font));
}

/// Read each shown face's caption keys, under its rig prefix, and let the
/// frame's time pass on every queue. A face no longer loaded leaves its
/// queue behind.
fn read_captions(
    time: Res<Time>,
    faces: Query<&Face, Without<Superseded>>,
    mut queues: ResMut<CaptionQueues>,
) {
    let text = |face: &Face, path: &str| match face
        .rig
        .value(&format!("{}{path}", face.meta.bundle.rig_prefix()))
    {
        Some(Value::String(text)) => text,
        _ => String::new(),
    };
    for face in &faces {
        let caption = text(face, standard::CAPTION_TEXT);
        let speaker = text(face, standard::CAPTION_SPEAKER);
        queues
            .0
            .entry(face.id.clone())
            .or_default()
            .read(&caption, &speaker);
    }
    queues
        .0
        .retain(|id, _| faces.iter().any(|face| &face.id == id));
    for queue in queues.0.values_mut() {
        queue.advance(time.delta_secs());
    }
}

/// Give every face an overlay on its camera, and take down the overlays of
/// faces gone.
fn keep_overlays(
    faces: Query<(Entity, &Face)>,
    overlays: Query<(Entity, &CaptionOverlay)>,
    font: Option<Res<CaptionFont>>,
    mut commands: Commands,
) {
    for (overlay, CaptionOverlay { face }) in &overlays {
        if faces.get(*face).is_err() {
            commands.entity(overlay).despawn();
        }
    }
    let Some(font) = font else { return };
    for (entity, face) in &faces {
        if overlays.iter().any(|(_, overlay)| overlay.face == entity) {
            continue;
        }
        commands.spawn((
            CaptionOverlay { face: entity },
            UiTargetCamera(face.camera),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(5.0),
                width: Val::Percent(90.0),
                bottom: Val::Vh(MARGIN_VH),
                height: Val::Vh(BAND_VH),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            children![(
                CaptionLine,
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.0.clone()),
                    font_size: FontSize::Vh(LINE_VH),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::splat(SHADOW_OFFSET),
                    color: Color::BLACK,
                },
                TextLayout::justify(Justify::Center),
                Pickable::IGNORE,
            )],
        ));
    }
}

/// Show each face's current chunk in its overlay, in its speaker's colour;
/// hide the overlay when nothing is showing.
fn show_captions(
    queues: Res<CaptionQueues>,
    faces: Query<(&Face, Has<Superseded>)>,
    mut overlays: Query<(&CaptionOverlay, &Children, &mut Visibility)>,
    mut lines: Query<(&mut Text, &mut TextColor), With<CaptionLine>>,
) {
    for (overlay, children, mut visibility) in &mut overlays {
        let showing: Option<&Chunk> = faces
            .get(overlay.face)
            .ok()
            .filter(|(_, superseded)| !superseded)
            .and_then(|(face, _)| queues.0.get(&face.id))
            .and_then(CaptionQueue::showing);
        let wanted = if showing.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let Some(chunk) = showing else { continue };
        for child in children.iter() {
            let Ok((mut text, mut color)) = lines.get_mut(child) else {
                continue;
            };
            if text.0 != chunk.text {
                text.0.clone_from(&chunk.text);
            }
            let [r, g, b] = chunk.color();
            let wanted = Color::srgb(r, g, b);
            if color.0 != wanted {
                color.0 = wanted;
            }
        }
    }
}
