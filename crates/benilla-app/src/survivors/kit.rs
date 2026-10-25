//! The menu's building blocks: one look for every tab of the main screen. Panels with an
//! ornamented title, tab and icon buttons that light up when selected, stat tiles, progress bars,
//! timeline chips, the big call-to-action button, and hover tooltips in the client's own
//! `GameTooltip` frame.
//!
//! Every builder spawns into a parent (`&mut ChildSpawnerCommands`) and sizes itself in the
//! 768-unit canvas the HUD lays out on (`hud::apply_ui_scale`). Buttons carry whatever action
//! component the caller gives them; a click is read back through [`KitClicks`], the way the glue
//! screens read [`crate::glue::GlueClicks`]:
//!
//! ```ignore
//! kit::icon_button(p, &fonts, IconSource::Spell(403), 48.0, Some("Shaman"), true, PickClass(6));
//! // later, in a system:
//! for (e, PickClass(c)) in &buttons { if clicks.hit(e) { … } }
//! ```
//!
//! `WOW_UI_KIT=1` (`?ui_kit=1` on the web) lays a gallery of every block over the menu.

use std::collections::HashMap;

use bevy::prelude::*;

use super::hud::{self, TipLine};
use super::Phase;

// ── The look ─────────────────────────────────────────────────────────────────────────────────

/// The menu's colours.
pub(super) mod theme {
    use bevy::prelude::Color;

    /// Titles and highlights.
    pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
    /// A softer gold for ornaments and resting borders.
    pub const BRASS: Color = Color::srgb(0.62, 0.5, 0.26);
    /// Body text.
    pub const TEXT: Color = Color::srgb(0.93, 0.89, 0.8);
    pub const DIM: Color = Color::srgb(0.62, 0.6, 0.55);
    /// Bonuses, as the screenshot's collection line.
    pub const GREEN: Color = Color::srgb(0.25, 0.9, 0.3);
    pub const RED: Color = Color::srgb(0.95, 0.3, 0.25);
    /// A panel's fill, top to bottom.
    pub const PANEL_TOP: Color = Color::srgba(0.11, 0.08, 0.05, 0.94);
    pub const PANEL_BOTTOM: Color = Color::srgba(0.05, 0.035, 0.025, 0.96);
    /// A tile or a resting button inside a panel.
    pub const INSET: Color = Color::srgba(0.03, 0.025, 0.02, 0.85);
    /// The deep red of the call-to-action buttons.
    pub const ACTION: Color = Color::srgba(0.36, 0.06, 0.04, 0.96);
    pub const ACTION_HOT: Color = Color::srgba(0.52, 0.1, 0.06, 0.98);
    /// A plain button.
    pub const BUTTON: Color = Color::srgba(0.12, 0.08, 0.05, 0.95);
    pub const BUTTON_HOT: Color = Color::srgba(0.22, 0.14, 0.07, 0.98);
    /// A locked entry.
    pub const LOCKED: Color = Color::srgba(0.06, 0.05, 0.05, 0.85);
}

/// The fonts the menu speaks: Friz Quadrata for text, Morpheus for titles (as the client's quest
/// titles), Arial Narrow for numbers.
#[derive(Resource, Clone)]
pub(super) struct KitFonts {
    pub body: Handle<Font>,
    pub title: Handle<Font>,
    pub number: Handle<Font>,
}

fn load_fonts(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(KitFonts {
        body: crate::char_select::wow_font(&assets),
        title: assets.load("mpq://Fonts/MORPHEUS.ttf"),
        number: assets.load("mpq://Fonts/ARIALN.ttf"),
    });
}

/// A text bundle in one of the kit's fonts.
pub(super) fn text(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.2),
            color: Color::BLACK,
        },
    )
}

// ── Interaction ──────────────────────────────────────────────────────────────────────────────

/// The colours a kit button wears at rest, hovered and selected.
#[derive(Component, Clone, Copy)]
pub(super) struct ButtonLook {
    pub rest: Color,
    pub hot: Color,
    pub edge: Color,
    pub edge_selected: Color,
}

impl ButtonLook {
    pub const PLAIN: Self = Self {
        rest: theme::BUTTON,
        hot: theme::BUTTON_HOT,
        edge: theme::BRASS,
        edge_selected: theme::GOLD,
    };
    pub const ACTION: Self = Self {
        rest: theme::ACTION,
        hot: theme::ACTION_HOT,
        edge: theme::GOLD,
        edge_selected: theme::GOLD,
    };
    /// A tab or an icon: no fill of its own.
    pub const BARE: Self = Self {
        rest: Color::NONE,
        hot: Color::srgba(1.0, 0.85, 0.4, 0.08),
        edge: Color::srgba(0.4, 0.33, 0.2, 0.8),
        edge_selected: theme::GOLD,
    };
}

/// A selected button: the gold edge and the glow. Toggled by the screen that owns it.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Selected(pub bool);

/// A button that cannot be pressed now: dimmed, and its clicks are not reported.
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Disabled(pub bool);

/// The kit buttons pressed this frame.
#[derive(Resource, Default)]
pub(super) struct KitClicks(Vec<Entity>);

impl KitClicks {
    pub fn hit(&self, e: Entity) -> bool {
        self.0.contains(&e)
    }

    /// The action of the first pressed button among `buttons`.
    pub fn first<'a, A: Component>(
        &self,
        buttons: impl IntoIterator<Item = (Entity, &'a A)>,
    ) -> Option<&'a A> {
        buttons
            .into_iter()
            .find(|(e, _)| self.hit(*e))
            .map(|(_, a)| a)
    }
}

#[allow(clippy::type_complexity)]
fn read_clicks(
    mut clicks: ResMut<KitClicks>,
    buttons: Query<
        (Entity, &Interaction, Option<&Disabled>),
        (Changed<Interaction>, With<ButtonLook>),
    >,
) {
    clicks.0.clear();
    for (e, i, disabled) in &buttons {
        if *i == Interaction::Pressed && !disabled.is_some_and(|d| d.0) {
            clicks.0.push(e);
        }
    }
}

/// The look follows hover, selection and disabled state.
#[allow(clippy::type_complexity)]
fn style_buttons(
    mut buttons: Query<
        (
            &Interaction,
            &ButtonLook,
            Option<&Selected>,
            Option<&Disabled>,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&mut BoxShadow>,
        ),
        Or<(
            Changed<Interaction>,
            Changed<Selected>,
            Changed<Disabled>,
            Added<ButtonLook>,
        )>,
    >,
) {
    for (i, look, selected, disabled, mut bg, mut edge, shadow) in &mut buttons {
        let selected = selected.is_some_and(|s| s.0);
        let disabled = disabled.is_some_and(|d| d.0);
        let (fill, rim) = button_colors(look, *i, selected, disabled);
        if bg.0 != fill {
            bg.0 = fill;
        }
        let rim = BorderColor::all(rim);
        if *edge != rim {
            *edge = rim;
        }
        if let Some(mut shadow) = shadow {
            let glow = if selected && !disabled {
                theme::GOLD.with_alpha(0.45)
            } else {
                Color::NONE
            };
            if let Some(s) = shadow.0.first_mut() {
                if s.color != glow {
                    s.color = glow;
                }
            }
        }
    }
}

/// `(fill, edge)` for a button's state.
fn button_colors(
    look: &ButtonLook,
    i: Interaction,
    selected: bool,
    disabled: bool,
) -> (Color, Color) {
    if disabled {
        return (
            look.rest.with_alpha(look.rest.alpha() * 0.5),
            theme::DIM.with_alpha(0.4),
        );
    }
    let fill = if i == Interaction::None {
        look.rest
    } else {
        look.hot
    };
    let edge = if selected || i != Interaction::None {
        look.edge_selected
    } else {
        look.edge
    };
    (fill, edge)
}

// ── Icons ────────────────────────────────────────────────────────────────────────────────────

/// What an icon shows: a spell's icon from `Spell.dbc`, or a texture by its path in the client's
/// archives (an item's `Interface\Icons\…`, a class or race icon).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum IconSource {
    Spell(u32),
    Texture(String),
    /// One cell of an atlas: its texcoords `[left, right, top, bottom]` in 0..1, as the client's
    /// `SetTexCoord` (the class icons' `CLASS_ICON_TCOORDS`).
    Atlas(String, [f32; 4]),
}

/// An icon slot filled from a texture path once the archives are readable.
#[derive(Component)]
pub(super) struct TextureIcon(pub String, pub Option<[f32; 4]>);

#[derive(Component)]
struct TextureIconDone;

fn fill_texture_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &TextureIcon), Without<TextureIconDone>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<String, Handle<Image>>>,
) {
    let Some(assets) = assets.as_deref_mut() else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let handle = match cache.get(&icon.0) {
            Some(h) => Some(h.clone()),
            None => {
                let path = if icon.0.to_ascii_lowercase().ends_with(".blp") {
                    icon.0.clone()
                } else {
                    format!("{}.blp", icon.0)
                };
                let h = assets.sprite_texture(&path, &mut images);
                if let Some(h) = &h {
                    cache.insert(icon.0.clone(), h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            if let Some(tc) = icon.1 {
                if let Some(size) = images.get(&h).map(|i| i.size_f32()) {
                    img.rect = Some(crate::glue::art::tc_rect(size, tc));
                }
            }
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(TextureIconDone);
    }
}

/// An icon image, filled once the client data is up.
pub(super) fn icon_image(p: &mut ChildSpawnerCommands, source: &IconSource, size: f32) -> Entity {
    let mut e = p.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        ImageNode {
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
            ..default()
        },
    ));
    match source {
        IconSource::Spell(id) => e.insert(hud::SpellIcon(*id)),
        IconSource::Texture(path) => e.insert(TextureIcon(path.clone(), None)),
        IconSource::Atlas(path, tc) => e.insert(TextureIcon(path.clone(), Some(*tc))),
    };
    e.id()
}

// ── Tooltips ─────────────────────────────────────────────────────────────────────────────────

/// Hovering this shows these lines in the `GameTooltip` frame, under it.
#[derive(Component)]
pub(super) struct Tooltip(pub Vec<TipLine>);

impl Tooltip {
    /// A title and a body line, the common case.
    pub fn simple(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self(vec![
            TipLine::new(title, Color::WHITE).header(),
            TipLine::new(body, theme::GOLD),
        ])
    }
}

#[derive(Component)]
struct KitTooltipFrame;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show_tooltips(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    ui_scale: Res<UiScale>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    hovered: Query<(
        Entity,
        &Interaction,
        Ref<Tooltip>,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mut frames: Query<(&mut Node, &mut Visibility), With<KitTooltipFrame>>,
    mut built: Local<Option<(Entity, Entity)>>,
    mut shown: Local<Option<Entity>>,
) {
    let Some(fonts) = fonts else { return };
    let Some((frame, body)) = *built else {
        if let Some(assets) = assets.as_deref_mut() {
            let (frame, body) = hud::spawn_tooltip(&mut commands, assets, &mut images, ui_scale.0);
            commands.entity(frame).insert(KitTooltipFrame);
            *built = Some((frame, body));
        }
        return;
    };
    let Ok((mut node, mut vis)) = frames.get_mut(frame) else {
        return;
    };
    let Some((e, _, tip, computed, at)) =
        hovered.iter().find(|(_, i, ..)| **i != Interaction::None)
    else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        *shown = None;
        return;
    };
    let inv = computed.inverse_scale_factor;
    let (left, top) = (
        Val::Px((at.translation.x - computed.size().x / 2.0) * inv),
        Val::Px((at.translation.y + computed.size().y / 2.0) * inv + 4.0),
    );
    if node.left != left || node.top != top {
        node.left = left;
        node.top = top;
    }
    if *shown == Some(e) && !tip.is_changed() {
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        return;
    }
    *shown = Some(e);
    // Shown from the next frame, once the new lines have laid out.
    *vis = Visibility::Hidden;
    hud::fill_tooltip(&mut commands, body, &fonts.body, tip.0.clone());
}

// ── Builders ─────────────────────────────────────────────────────────────────────────────────

/// A panel: the dark gradient in a brass frame, an ornamented title on top if given, and the
/// body the caller fills.
pub(super) fn panel(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    title: Option<&str>,
    node: Node,
    body: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(14.0)),
            row_gap: Val::Px(8.0),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..node
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(theme::PANEL_TOP, 0.0),
            ColorStop::percent(theme::PANEL_BOTTOM, 100.0),
        ])),
        BorderColor::all(theme::BRASS),
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.6),
            Val::Px(0.0),
            Val::Px(4.0),
            Val::Px(0.0),
            Val::Px(12.0),
        ),
    ))
    .with_children(|p| {
        if let Some(title) = title {
            heading(p, fonts, title);
        }
        body(p);
    })
    .id()
}

/// A section title in Morpheus between two fading gold rules, as the screenshot's "Class",
/// "Equipment" and "Zone".
pub(super) fn heading(p: &mut ChildSpawnerCommands, fonts: &KitFonts, title: &str) -> Entity {
    let rule = |to_left: bool| {
        let (a, b) = (theme::BRASS.with_alpha(0.0), theme::BRASS);
        let stops = if to_left {
            vec![ColorStop::percent(b, 0.0), ColorStop::percent(a, 100.0)]
        } else {
            vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)]
        };
        (
            Node {
                flex_grow: 1.0,
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_right(stops)),
        )
    };
    p.spawn(Node {
        width: Val::Percent(100.0),
        align_items: AlignItems::Center,
        column_gap: Val::Px(12.0),
        margin: UiRect::vertical(Val::Px(2.0)),
        ..default()
    })
    .with_children(|r| {
        r.spawn(rule(false));
        r.spawn(text(&fonts.title, title, 22.0, theme::GOLD));
        r.spawn(rule(true));
    })
    .id()
}

/// A row of tabs: one per label, each carrying the caller's action; `active` is the selected one.
/// Returns the tabs, in order, so the caller can disable or annotate them.
pub(super) fn tab_bar<A: Component + Clone>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    tabs: &[(&str, A)],
    active: usize,
) -> Vec<Entity> {
    let mut ids = Vec::with_capacity(tabs.len());
    p.spawn(Node {
        column_gap: Val::Px(6.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|r| {
        for (i, (label, action)) in tabs.iter().enumerate() {
            let mut tab = r.spawn((
                Button,
                ButtonLook::PLAIN,
                Selected(i == active),
                Disabled(false),
                action.clone(),
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(theme::BUTTON),
                BorderColor::all(theme::BRASS),
                BoxShadow::new(
                    Color::NONE,
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(8.0),
                ),
            ));
            tab.with_children(|b| {
                b.spawn(text(&fonts.body, *label, 17.0, theme::GOLD));
            });
            ids.push(tab.id());
        }
    });
    ids
}

/// A button with a label and an optional line under it ("Run Options / Normal difficulty").
pub(super) fn button<A: Bundle>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    label: &str,
    sub: Option<&str>,
    look: ButtonLook,
    action: A,
) -> Entity {
    labelled_button(p, fonts, label, 18.0, sub, look, action)
}

fn labelled_button<A: Bundle>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    label: &str,
    label_size: f32,
    sub: Option<&str>,
    look: ButtonLook,
    action: A,
) -> Entity {
    p.spawn((
        Button,
        look,
        Selected(false),
        Disabled(false),
        action,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(look.rest),
        BorderColor::all(look.edge),
        BoxShadow::new(
            Color::NONE,
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(10.0),
        ),
    ))
    .with_children(|b| {
        b.spawn(text(&fonts.title, label, label_size, theme::GOLD));
        if let Some(sub) = sub {
            b.spawn(text(&fonts.body, sub, 11.0, theme::TEXT));
        }
    })
    .id()
}

/// The screen's big call to action ("Enter Elwynn Forest"): a title in Morpheus and a line
/// under it, in deep red with a gold frame.
pub(super) fn big_button<A: Bundle>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    title: &str,
    sub: &str,
    action: A,
) -> Entity {
    labelled_button(p, fonts, title, 30.0, Some(sub), ButtonLook::ACTION, action)
}

/// A square icon button, with an optional label under it (the class grid). Returns the button
/// itself, the one that takes hovers and clicks, for a tooltip or a state.
pub(super) fn icon_button<A: Bundle>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    icon: IconSource,
    size: f32,
    label: Option<&str>,
    selected: bool,
    action: A,
) -> Entity {
    let mut button = Entity::PLACEHOLDER;
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(3.0),
        ..default()
    })
    .with_children(|c| {
        button = c
            .spawn((
                Button,
                ButtonLook::BARE,
                Selected(selected),
                Disabled(false),
                action,
                Node {
                    width: Val::Px(size + 4.0),
                    height: Val::Px(size + 4.0),
                    padding: UiRect::all(Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(ButtonLook::BARE.edge),
                BoxShadow::new(
                    Color::NONE,
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(2.0),
                    Val::Px(10.0),
                ),
            ))
            .with_children(|b| {
                icon_image(b, &icon, size);
            })
            .id();
        if let Some(label) = label {
            c.spawn(text(
                &fonts.body,
                label,
                12.0,
                if selected { theme::GOLD } else { theme::TEXT },
            ));
        }
    });
    button
}

/// A plain icon, not a button (a spell pool, a racial), with a tooltip if given.
pub(super) fn icon(
    p: &mut ChildSpawnerCommands,
    icon: IconSource,
    size: f32,
    tip: Option<Tooltip>,
) -> Entity {
    let mut e = p.spawn((
        Node {
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
    ));
    e.with_children(|b| {
        icon_image(b, &icon, size);
    });
    if let Some(tip) = tip {
        e.insert((tip, Interaction::default(), Button));
    }
    e.id()
}

/// A wrapping grid of `columns` cells, `gap` apart, filled by the caller.
pub(super) fn grid(
    p: &mut ChildSpawnerCommands,
    columns: u16,
    gap: f32,
    cells: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    p.spawn(Node {
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::auto(columns),
        column_gap: Val::Px(gap),
        row_gap: Val::Px(gap),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(cells)
    .id()
}

/// A small labelled number ("HEALTH / 195"). Its value is the second child, for updates.
pub(super) fn stat_tile(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    label: &str,
    value: &str,
) -> Entity {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            row_gap: Val::Px(1.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::INSET),
        BorderColor::all(Color::srgba(0.4, 0.33, 0.2, 0.6)),
    ))
    .with_children(|t| {
        t.spawn(text(&fonts.body, label.to_uppercase(), 9.0, theme::DIM));
        t.spawn((StatValue, text(&fonts.body, value, 15.0, theme::TEXT)));
    })
    .id()
}

/// A [`stat_tile`]'s number.
#[derive(Component)]
pub(super) struct StatValue;

/// A progress bar's numbers; change them and the bar follows.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(super) struct Progress {
    pub value: f32,
    pub max: f32,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        if self.max > 0.0 {
            (self.value / self.max).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    pub fn label(&self) -> String {
        format!("{} / {}", self.value.round(), self.max.round())
    }
}

#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct ProgressCount;

/// A labelled progress bar ("COLLECTION · 5 / 60"): the label left, the count right, a gold fill.
pub(super) fn progress_bar(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    label: &str,
    progress: Progress,
) -> Entity {
    p.spawn((
        progress,
        Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(3.0),
            ..default()
        },
    ))
    .with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|r| {
            r.spawn(text(&fonts.body, label.to_uppercase(), 10.0, theme::TEXT));
            r.spawn((
                ProgressCount,
                text(&fonts.number, progress.label(), 11.0, theme::GOLD),
            ));
        });
        c.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|track| {
            track.spawn((
                ProgressFill,
                Node {
                    width: Val::Percent(progress.fraction() * 100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_right(vec![
                    ColorStop::percent(theme::BRASS, 0.0),
                    ColorStop::percent(theme::GOLD, 100.0),
                ])),
            ));
        });
    })
    .id()
}

fn refresh_progress(
    bars: Query<(Entity, &Progress), Changed<Progress>>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, With<ProgressFill>>,
    mut counts: Query<&mut Text, With<ProgressCount>>,
) {
    for (bar, progress) in &bars {
        for e in children.iter_descendants(bar) {
            if let Ok(mut n) = fills.get_mut(e) {
                let w = Val::Percent(progress.fraction() * 100.0);
                if n.width != w {
                    n.width = w;
                }
            }
            if let Ok(mut t) = counts.get_mut(e) {
                let s = progress.label();
                if t.0 != s {
                    t.0 = s;
                }
            }
        }
    }
}

/// A small rounded tag ("3:00 · Hogger"); `highlight` rings it in gold (the final boss).
pub(super) fn chip(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    label: &str,
    highlight: bool,
) -> Entity {
    p.spawn((
        Node {
            padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(9.0)),
            ..default()
        },
        BackgroundColor(theme::INSET),
        BorderColor::all(if highlight {
            theme::GOLD
        } else {
            theme::BRASS.with_alpha(0.7)
        }),
    ))
    .with_children(|c| {
        c.spawn(text(
            &fonts.body,
            label,
            11.0,
            if highlight { theme::GOLD } else { theme::TEXT },
        ));
    })
    .id()
}

/// A line of bonuses in green, joined by " · " ("+25 max health · +1% critical strike").
pub(super) fn bonus_line(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    bonuses: &[String],
) -> Entity {
    p.spawn(text(&fonts.body, bonuses.join(" · "), 11.0, theme::GREEN))
        .id()
}

// ── Scrolling ────────────────────────────────────────────────────────────────────────────────

/// A column that scrolls with the mouse wheel while the pointer is over it, rows and buttons
/// included, with a bar on its right edge that shows where it is and drags.
#[derive(Component)]
pub(super) struct ScrollArea;

/// A scroll area's bar, and the thumb in it; both name the area they follow.
#[derive(Component)]
struct ScrollTrack(Entity);
#[derive(Component)]
struct ScrollThumb(Entity);

/// Wheel lines to pixels: a notch moves about two rows.
const SCROLL_LINE: f32 = 36.0;
/// The bar's width, and the room the rows leave it.
const SCROLL_BAR: f32 = 5.0;
const SCROLL_GUTTER: f32 = 10.0;
/// The thumb's least share of the bar, so a long list's stays easy to grab.
const THUMB_MIN: f32 = 0.08;

/// A scrolling column, filled by the caller; `node` sets its size and its rows' gap. Returns the
/// column the rows are in.
pub(super) fn scroll_area(
    p: &mut ChildSpawnerCommands,
    node: Node,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    let gap = node.row_gap;
    let mut area = Entity::PLACEHOLDER;
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        // A flex child shrinks below its content only with this; without it the area grows to
        // fit and never scrolls.
        min_height: Val::Px(0.0),
        ..node
    })
    .with_children(|o| {
        area = o
            .spawn((
                ScrollArea,
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    row_gap: gap,
                    padding: UiRect::right(Val::Px(SCROLL_GUTTER)),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .with_children(children)
            .id();
        o.spawn((
            ScrollTrack(area),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(SCROLL_BAR),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(theme::INSET),
            Visibility::Hidden,
        ))
        .with_children(|t| {
            t.spawn((
                ScrollThumb(area),
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(theme::BRASS),
            ));
        });
    });
    area
}

/// How far a scroll area may scroll, in logical pixels: its content past its own height.
fn scroll_room(node: &ComputedNode) -> f32 {
    ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0)
}

/// The thumb's top and height, as shares of the bar, for a view `size` tall over `content`
/// scrolled `at` of `room`; `None` when everything fits.
fn thumb_geometry(at: f32, room: f32, size: f32, content: f32) -> Option<(f32, f32)> {
    if room <= 0.5 || size <= 0.0 || content <= size {
        return None;
    }
    let shown = (size / content).clamp(THUMB_MIN, 1.0);
    let along = (at / room).clamp(0.0, 1.0);
    Some((along * (1.0 - shown), shown))
}

/// The wheel scrolls the area under the pointer, and a held thumb drags its area.
#[allow(clippy::type_complexity)]
fn scroll_areas(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut areas: Query<
        (
            &mut ScrollPosition,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        ),
        With<ScrollArea>,
    >,
    thumbs: Query<(&ScrollThumb, &Interaction)>,
    // The area dragged, the pointer's height when it was grabbed, and the scroll then.
    mut drag: Local<Option<(Entity, f32, f32)>>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let cursor = windows
        .single()
        .ok()
        .and_then(|w| w.physical_cursor_position());
    let dy: f32 = wheel
        .read()
        .map(|w| match w.unit {
            MouseScrollUnit::Line => w.y * SCROLL_LINE,
            MouseScrollUnit::Pixel => w.y,
        })
        .sum();
    if let (Some(c), true) = (cursor, dy != 0.0) {
        // Over the area's rectangle, not its `Interaction`: the rows' buttons take the hover,
        // and the area under them never sees it.
        for (mut pos, node, at) in &mut areas {
            let half = node.size() * 0.5;
            if (c - at.translation).abs().cmpgt(half).any() {
                continue;
            }
            let y = (pos.0.y - dy).clamp(0.0, scroll_room(node));
            if pos.0.y != y {
                pos.0.y = y;
            }
        }
    }
    let held = thumbs
        .iter()
        .find(|(_, i)| **i == Interaction::Pressed)
        .map(|(t, _)| t.0);
    let (Some(area), Some(c)) = (held, cursor) else {
        *drag = None;
        return;
    };
    let Ok((mut pos, node, _)) = areas.get_mut(area) else {
        return;
    };
    let (_, from_y, from_scroll) = *drag.get_or_insert((area, c.y, pos.0.y));
    let (size, content) = (node.size().y, node.content_size().y);
    if size <= 0.0 {
        return;
    }
    // A pixel of thumb travel is `content / size` pixels of the list.
    let moved = (c.y - from_y) * content / size * node.inverse_scale_factor();
    let y = (from_scroll + moved).clamp(0.0, scroll_room(node));
    if pos.0.y != y {
        pos.0.y = y;
    }
}

/// Each bar shows while its list overflows, its thumb as long as the share in view and as far
/// down as the list is scrolled.
fn show_scrollbars(
    areas: Query<(&ScrollPosition, &ComputedNode), With<ScrollArea>>,
    mut tracks: Query<(&ScrollTrack, &mut Visibility)>,
    mut thumbs: Query<(&ScrollThumb, &mut Node)>,
) {
    let geometry = |area: Entity| {
        let (pos, n) = areas.get(area).ok()?;
        thumb_geometry(pos.0.y, scroll_room(n), n.size().y, n.content_size().y)
    };
    for (track, mut vis) in &mut tracks {
        let want = if geometry(track.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    for (thumb, mut node) in &mut thumbs {
        let Some((top, height)) = geometry(thumb.0) else {
            continue;
        };
        let (top, height) = (Val::Percent(top * 100.0), Val::Percent(height * 100.0));
        if node.top != top {
            node.top = top;
        }
        if node.height != height {
            node.height = height;
        }
    }
}

// ── Text fields ──────────────────────────────────────────────────────────────────────────────

/// A one-line text field: click it to type, then Enter, Escape, Tab or a click elsewhere to
/// finish, which sends [`FieldDone`]. `accept` filters what can be typed.
#[derive(Component)]
pub(super) struct TextField {
    pub value: String,
    pub max: usize,
    pub accept: fn(char) -> bool,
}

/// The field being typed in, if any. While it is set, screens leave the keyboard alone: Enter
/// does not start a run, G and C do not toggle anything.
#[derive(Resource, Default)]
pub(super) struct KitFocus(pub Option<Entity>);

impl KitFocus {
    pub fn typing(&self) -> bool {
        self.0.is_some()
    }
}

/// A field was finished, with this value.
#[derive(Message, Clone, Debug, PartialEq)]
pub(super) struct FieldDone {
    pub field: Entity,
    pub value: String,
}

#[derive(Component)]
struct FieldText;

/// The look of a field's box.
const FIELD: ButtonLook = ButtonLook {
    rest: Color::srgba(0.02, 0.015, 0.01, 0.92),
    hot: Color::srgba(0.06, 0.045, 0.02, 0.95),
    edge: theme::BRASS,
    edge_selected: theme::GOLD,
};

/// A text field holding `value`, `width` wide, with the caller's components on it.
pub(super) fn text_field<A: Bundle>(
    p: &mut ChildSpawnerCommands,
    fonts: &KitFonts,
    value: &str,
    width: f32,
    max: usize,
    accept: fn(char) -> bool,
    extra: A,
) -> Entity {
    p.spawn((
        Button,
        FIELD,
        Selected(false),
        Disabled(false),
        TextField {
            value: value.to_string(),
            max,
            accept,
        },
        extra,
        Node {
            width: Val::Px(width),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(FIELD.rest),
        BorderColor::all(FIELD.edge),
    ))
    .with_children(|b| {
        b.spawn((FieldText, text(&fonts.body, value, 20.0, Color::WHITE)));
    })
    .id()
}

/// A click on a field focuses it; a click anywhere else, or Enter, Escape or Tab, finishes it.
#[allow(clippy::too_many_arguments)]
fn edit_fields(
    clicks: Res<KitClicks>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut focus: ResMut<KitFocus>,
    mut fields: Query<(Entity, &mut TextField, &mut Selected)>,
    mut done: MessageWriter<FieldDone>,
) {
    use bevy::input::keyboard::Key;
    use bevy::input::ButtonState;

    // A focused field that went away lets the keyboard go.
    if let Some(f) = focus.0 {
        if fields.get(f).is_err() {
            focus.0 = None;
        }
    }
    let clicked = fields.iter().map(|(e, ..)| e).find(|e| clicks.hit(*e));
    let finish = |focus: &mut KitFocus,
                  fields: &mut Query<(Entity, &mut TextField, &mut Selected)>,
                  done: &mut MessageWriter<FieldDone>| {
        if let Some(f) = focus.0.take() {
            if let Ok((_, field, mut sel)) = fields.get_mut(f) {
                sel.0 = false;
                done.write(FieldDone {
                    field: f,
                    value: field.value.clone(),
                });
            }
        }
    };
    if let Some(f) = clicked {
        if focus.0 != Some(f) {
            finish(&mut focus, &mut fields, &mut done);
            focus.0 = Some(f);
            if let Ok((_, _, mut sel)) = fields.get_mut(f) {
                sel.0 = true;
            }
        }
    } else if focus.0.is_some() && mouse.just_pressed(MouseButton::Left) {
        finish(&mut focus, &mut fields, &mut done);
    }
    let Some(f) = focus.0 else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Enter | Key::Escape | Key::Tab => {
                finish(&mut focus, &mut fields, &mut done);
                break;
            }
            Key::Backspace => {
                if let Ok((_, mut field, _)) = fields.get_mut(f) {
                    field.value.pop();
                }
            }
            _ => {
                if let (Some(t), Ok((_, mut field, _))) = (&ev.text, fields.get_mut(f)) {
                    for c in t.chars().filter(|c| !c.is_control()) {
                        if (field.accept)(c) && field.value.chars().count() < field.max {
                            field.value.push(c);
                        }
                    }
                }
            }
        }
    }
}

/// A field shows its value, with a caret while it is typed in.
fn show_fields(
    focus: Res<KitFocus>,
    fields: Query<(Entity, Ref<TextField>, &Children)>,
    mut texts: Query<&mut Text, With<FieldText>>,
) {
    for (e, field, children) in &fields {
        if !field.is_changed() && !focus.is_changed() {
            continue;
        }
        let shown = if focus.0 == Some(e) {
            format!("{}|", field.value)
        } else {
            field.value.clone()
        };
        for c in children.iter() {
            if let Ok(mut t) = texts.get_mut(c) {
                if t.0 != shown {
                    t.0.clone_from(&shown);
                }
            }
        }
    }
}

// ── Wiring ───────────────────────────────────────────────────────────────────────────────────

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<KitClicks>()
        .init_resource::<KitFocus>()
        .add_message::<FieldDone>()
        .add_systems(Startup, load_fonts)
        .add_systems(
            PreUpdate,
            (read_clicks, edit_fields)
                .chain()
                .after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(
            Update,
            (
                style_buttons,
                fill_texture_icons,
                refresh_progress,
                show_tooltips,
                show_fields,
                scroll_areas,
                show_scrollbars,
            ),
        );
    if crate::webenv::var("WOW_UI_KIT").is_some_and(|v| v != "0") {
        app.add_systems(Update, gallery::show);
    }
}

/// `WOW_UI_KIT=1`: every block on one screen over the menu, to judge the look in the game.
mod gallery {
    use super::*;

    #[derive(Component, Clone)]
    pub(super) struct Demo(u8);

    #[derive(Component)]
    pub(super) struct Gallery;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn show(
        mut commands: Commands,
        phase: Res<State<Phase>>,
        fonts: Option<Res<KitFonts>>,
        clicks: Res<KitClicks>,
        buttons: Query<(Entity, &Demo)>,
        mut selected: Query<(&Demo, &mut Selected)>,
        mut bars: Query<&mut Progress>,
        root: Query<(), With<Gallery>>,
    ) {
        let Some(fonts) = fonts else { return };
        if *phase.get() != Phase::Menu {
            return;
        }
        if root.is_empty() {
            spawn(&mut commands, &fonts);
            return;
        }
        if let Some(Demo(n)) = clicks.first(&buttons) {
            for (d, mut s) in &mut selected {
                let want = Selected(d.0 == *n);
                if *s != want {
                    *s = want;
                }
            }
            for mut b in &mut bars {
                b.value = (b.value + 1.0) % (b.max + 1.0);
            }
        }
    }

    fn spawn(commands: &mut Commands, fonts: &KitFonts) {
        commands
            .spawn((
                Gallery,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                GlobalZIndex(470),
            ))
            .with_children(|root| {
                panel(
                    root,
                    fonts,
                    Some("Ui Kit"),
                    Node {
                        width: Val::Px(520.0),
                        ..default()
                    },
                    |p| {
                        tab_bar(
                            p,
                            fonts,
                            &[
                                ("Character", Demo(10)),
                                ("Collection", Demo(11)),
                                ("Talents", Demo(12)),
                            ],
                            0,
                        );
                        heading(p, fonts, "Class");
                        grid(p, 3, 10.0, |g| {
                            for (i, (name, spell)) in
                                [("Warrior", 100u32), ("Mage", 133), ("Shaman", 403)]
                                    .into_iter()
                                    .enumerate()
                            {
                                icon_button(
                                    g,
                                    fonts,
                                    IconSource::Spell(spell),
                                    40.0,
                                    Some(name),
                                    i == 2,
                                    Demo(i as u8),
                                );
                            }
                        });
                        p.spawn(Node {
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|r| {
                            stat_tile(r, fonts, "Health", "195");
                            stat_tile(r, fonts, "Starts with", "Lightning Bolt");
                            stat_tile(r, fonts, "Zones open", "2 of 12");
                        });
                        p.spawn(Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|r| {
                            for spell in [403u32, 421, 8050, 2484] {
                                icon(
                                    r,
                                    IconSource::Spell(spell),
                                    24.0,
                                    Some(Tooltip::simple("A spell", "Its tooltip line.")),
                                );
                            }
                        });
                        progress_bar(
                            p,
                            fonts,
                            "Collection",
                            Progress {
                                value: 5.0,
                                max: 60.0,
                            },
                        );
                        bonus_line(
                            p,
                            fonts,
                            &[
                                "+25 max health".into(),
                                "+1% critical strike".into(),
                                "+5% movement speed".into(),
                            ],
                        );
                        p.spawn(Node {
                            column_gap: Val::Px(6.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|r| {
                            chip(r, fonts, "3:00 · Hogger", false);
                            chip(r, fonts, "16:00 · Edwin VanCleef", true);
                        });
                        p.spawn(Node {
                            column_gap: Val::Px(10.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|r| {
                            button(
                                r,
                                fonts,
                                "Run Options",
                                Some("Normal difficulty"),
                                ButtonLook::PLAIN,
                                Demo(20),
                            );
                            big_button(
                                r,
                                fonts,
                                "Enter Elwynn Forest",
                                "The Defias Uprising",
                                Demo(21),
                            );
                            button(
                                r,
                                fonts,
                                "Co-op",
                                Some("Play with friends"),
                                ButtonLook::PLAIN,
                                Demo(22),
                            );
                        });
                    },
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<KitClicks>()
            .insert_resource(KitFonts {
                body: Handle::default(),
                title: Handle::default(),
                number: Handle::default(),
            })
            .add_systems(
                Update,
                (read_clicks, style_buttons, refresh_progress).chain(),
            );
        app
    }

    #[derive(Component, Clone, Debug, PartialEq)]
    struct Act(u8);

    #[test]
    fn the_thumb_shows_the_share_in_view_and_where_it_is() {
        // Everything fits: no bar.
        assert_eq!(thumb_geometry(0.0, 0.0, 400.0, 300.0), None);
        // Twice the view: half the bar, at the top, then at the bottom.
        assert_eq!(thumb_geometry(0.0, 400.0, 400.0, 800.0), Some((0.0, 0.5)));
        assert_eq!(thumb_geometry(400.0, 400.0, 400.0, 800.0), Some((0.5, 0.5)));
        // A long list's thumb keeps a grabbable length and still reaches the bottom.
        let (top, height) = thumb_geometry(9_600.0, 9_600.0, 400.0, 10_000.0).unwrap();
        assert_eq!(height, THUMB_MIN);
        assert!((top + height - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_scroll_area_carries_its_bar() {
        let mut app = app();
        build(&mut app, |p, _| {
            scroll_area(p, Node::default(), |_| {});
        });
        app.update();
        let w = app.world_mut();
        let area = w
            .query_filtered::<Entity, With<ScrollArea>>()
            .single(w)
            .unwrap();
        let mut thumbs = w.query::<&ScrollThumb>();
        assert!(
            thumbs.iter(w).any(|t| t.0 == area),
            "the thumb follows its area"
        );
        let mut tracks = w.query::<&ScrollTrack>();
        assert!(tracks.iter(w).any(|t| t.0 == area));
    }

    fn build(
        app: &mut App,
        f: impl FnOnce(&mut ChildSpawnerCommands, &KitFonts) + Send + Sync + 'static,
    ) -> Entity {
        let fonts = app.world().resource::<KitFonts>().clone();
        let root = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .commands()
            .entity(root)
            .with_children(|p| f(p, &fonts));
        app.world_mut().flush();
        root
    }

    #[test]
    fn a_press_is_reported_once_and_a_disabled_button_stays_silent() {
        let mut app = app();
        let root = build(&mut app, |p, f| {
            button(p, f, "Go", None, ButtonLook::PLAIN, Act(1));
            button(p, f, "No", None, ButtonLook::PLAIN, Act(2));
        });
        let kids: Vec<Entity> = app.world().get::<Children>(root).unwrap().iter().collect();
        app.world_mut().get_mut::<Disabled>(kids[1]).unwrap().0 = true;
        for &k in &kids {
            *app.world_mut().get_mut::<Interaction>(k).unwrap() = Interaction::Pressed;
        }
        app.update();
        let clicks = app.world().resource::<KitClicks>();
        assert!(clicks.hit(kids[0]));
        assert!(!clicks.hit(kids[1]));
        // Held, not pressed again: no new click.
        app.update();
        assert!(!app.world().resource::<KitClicks>().hit(kids[0]));
    }

    #[test]
    fn first_finds_the_pressed_action() {
        let mut app = app();
        let root = build(&mut app, |p, f| {
            button(p, f, "A", None, ButtonLook::PLAIN, Act(7));
            button(p, f, "B", None, ButtonLook::PLAIN, Act(8));
        });
        let kids: Vec<Entity> = app.world().get::<Children>(root).unwrap().iter().collect();
        *app.world_mut().get_mut::<Interaction>(kids[1]).unwrap() = Interaction::Pressed;
        app.update();
        let mut q = app.world_mut().query::<(Entity, &Act)>();
        let world = app.world();
        let clicks = world.resource::<KitClicks>();
        assert_eq!(clicks.first(q.iter(world)), Some(&Act(8)));
    }

    #[test]
    fn selection_and_hover_change_the_look() {
        let look = ButtonLook::PLAIN;
        let (rest, edge) = button_colors(&look, Interaction::None, false, false);
        assert_eq!((rest, edge), (look.rest, look.edge));
        let (hot, _) = button_colors(&look, Interaction::Hovered, false, false);
        assert_eq!(hot, look.hot);
        let (_, sel) = button_colors(&look, Interaction::None, true, false);
        assert_eq!(sel, look.edge_selected);
        let (dim, _) = button_colors(&look, Interaction::Hovered, true, true);
        assert!(dim.alpha() < look.rest.alpha());

        let mut app = app();
        let root = build(&mut app, |p, f| {
            icon_button(
                p,
                f,
                IconSource::Spell(403),
                40.0,
                Some("Shaman"),
                false,
                Act(3),
            );
        });
        app.update();
        let column = app.world().get::<Children>(root).unwrap()[0];
        let button = app.world().get::<Children>(column).unwrap()[0];
        app.world_mut().get_mut::<Selected>(button).unwrap().0 = true;
        app.update();
        let shadow = app.world().get::<BoxShadow>(button).unwrap();
        assert_ne!(shadow.0[0].color, Color::NONE, "a selected icon glows");
        assert_eq!(
            *app.world().get::<BorderColor>(button).unwrap(),
            BorderColor::all(ButtonLook::BARE.edge_selected)
        );
    }

    #[test]
    fn a_progress_bar_follows_its_numbers() {
        let mut app = app();
        let root = build(&mut app, |p, f| {
            progress_bar(
                p,
                f,
                "Collection",
                Progress {
                    value: 5.0,
                    max: 60.0,
                },
            );
        });
        let bar = app.world().get::<Children>(root).unwrap()[0];
        app.update();
        app.world_mut().get_mut::<Progress>(bar).unwrap().value = 30.0;
        app.update();
        let mut fills = app
            .world_mut()
            .query_filtered::<&Node, With<ProgressFill>>();
        let w = fills.iter(app.world()).next().unwrap().width;
        assert_eq!(w, Val::Percent(50.0));
        let mut counts = app
            .world_mut()
            .query_filtered::<&Text, With<ProgressCount>>();
        assert_eq!(counts.iter(app.world()).next().unwrap().0, "30 / 60");
    }

    fn field_app() -> App {
        let mut app = app();
        app.init_resource::<KitFocus>()
            .add_message::<FieldDone>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(
                Update,
                (edit_fields, show_fields).chain().after(read_clicks),
            );
        app
    }

    fn press(app: &mut App, key: bevy::input::keyboard::Key, text: Option<&str>) {
        use bevy::input::keyboard::{KeyCode, KeyboardInput};
        use bevy::input::ButtonState;
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: key,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }

    #[test]
    fn a_field_takes_typing_until_enter_and_reports_its_value() {
        use bevy::input::keyboard::Key;
        let mut app = field_app();
        let root = build(&mut app, |p, f| {
            text_field(p, f, "Zul", 200.0, 6, |c| c.is_alphabetic(), Act(9));
        });
        let field = app.world().get::<Children>(root).unwrap()[0];
        // Not focused: typing goes nowhere.
        press(&mut app, Key::Character("x".into()), Some("x"));
        app.update();
        assert!(!app.world().resource::<KitFocus>().typing());
        assert_eq!(app.world().get::<TextField>(field).unwrap().value, "Zul");
        // Click to focus, type, a digit is refused and the length is capped.
        *app.world_mut().get_mut::<Interaction>(field).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<KitFocus>().typing());
        for t in ["j", "1", "i", "n", "n", "n"] {
            press(&mut app, Key::Character(t.into()), Some(t));
        }
        app.update();
        assert_eq!(app.world().get::<TextField>(field).unwrap().value, "Zuljin");
        press(&mut app, Key::Backspace, None);
        app.update();
        assert_eq!(app.world().get::<TextField>(field).unwrap().value, "Zulji");
        // Enter finishes: the focus goes and the value is reported.
        press(&mut app, Key::Enter, None);
        app.update();
        assert!(!app.world().resource::<KitFocus>().typing());
        let done: Vec<FieldDone> = app
            .world()
            .resource::<Messages<FieldDone>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert_eq!(
            done,
            vec![FieldDone {
                field,
                value: "Zulji".into()
            }]
        );
    }

    #[test]
    fn progress_handles_an_empty_max() {
        let p = Progress {
            value: 3.0,
            max: 0.0,
        };
        assert_eq!(p.fraction(), 0.0);
        let full = Progress {
            value: 99.0,
            max: 60.0,
        };
        assert_eq!(full.fraction(), 1.0);
    }

    #[test]
    fn icons_pick_their_filler() {
        let mut app = app();
        let root = build(&mut app, |p, _| {
            icon_image(p, &IconSource::Spell(403), 24.0);
            icon_image(
                p,
                &IconSource::Texture("Interface\\Icons\\INV_Sword_04".into()),
                24.0,
            );
        });
        let kids: Vec<Entity> = app.world().get::<Children>(root).unwrap().iter().collect();
        assert!(app.world().get::<hud::SpellIcon>(kids[0]).is_some());
        assert!(app.world().get::<TextureIcon>(kids[1]).is_some());
        assert!(app.world().get::<TextureIcon>(kids[0]).is_none());
    }
}
