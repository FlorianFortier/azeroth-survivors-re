//! The field's marks, read off the replicated units so a guest sees what the host sees: a ring on
//! the ground under each chest and elite, and a dashed one at a fallen elite's post that fills as
//! it comes back; over the world, a chest's price and an elite's reward with its icon, or the
//! seconds until a post's elite returns. The minimap's own marks are drawn in `minimap.rs`.
//!
//! Chests and elites carry a [`FieldMark`] on every machine (the host sets it, a guest reads it
//! off the spawn's role), and the posts are [`Run::posts`] (the host's stream, for a guest), so
//! nothing here runs the simulation.

use std::collections::HashMap;

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::data::{self, Reward};
use super::hud::{HudFont, HudRoot, SpellIcon};
use super::lang::{tr, trf};
use super::treasure::{FieldMark, ShrineState};
use super::units::is_dead;
use super::{LocalHero, Phase, Run};
use crate::net::ObjectStore;

/// Segments of a ground ring.
const RING_SEGMENTS: usize = 40;
/// Off the ground, so the line clears the terrain it follows.
const RING_LIFT: f32 = 0.15;
/// An elite's ring, in yards for a scale-1 elite.
const ELITE_RING: f32 = 2.2;
/// The label's box, wide enough for its longest line; the text centres in it.
const LABEL_W: f32 = 240.0;
const LABEL_ICON: f32 = 22.0;
const LABEL_TITLE: f32 = 15.0;
const LABEL_LINE: f32 = 13.0;

/// What a label is over: a live chest or elite, or an elite's empty post.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Key {
    Unit(Entity),
    Post(usize),
    Shrine(usize),
}

/// One label's nodes.
pub(super) struct Label {
    root: Entity,
    title: Entity,
    line: Entity,
}

/// What one label shows this frame.
#[derive(Debug, PartialEq)]
struct Shown {
    at: Vec3,
    icon: u32,
    title: String,
    line: String,
    color: [f32; 3],
}

fn srgb(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn dim(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| v * k)
}

/// What an elite guards, as its label's second line says it.
pub(super) fn reward_text(reward: Reward) -> String {
    match reward {
        Reward::Gear { percent } => trf("Gear {}%", &[&percent.to_string()]),
        Reward::SpellRank => tr("Spell rank").to_string(),
        Reward::RunItem => tr("Run item").to_string(),
        Reward::Gold(n) => trf("{} gold", &[&n.to_string()]),
    }
}

/// Whole seconds left on a post's timer, as the countdown reads them.
fn seconds_left(wait: f32) -> u32 {
    wait.max(0.0).ceil() as u32
}

/// How far a post's elite is on its way back, 0 when it has just fallen, 1 when it is back.
fn returning(kind: usize, wait: f32) -> f32 {
    let total = data::ELITES[kind.min(data::ELITES.len() - 1)].respawn;
    if total <= 0.0 {
        return 1.0;
    }
    (1.0 - wait / total).clamp(0.0, 1.0)
}

/// Whether the field's marks show: a run in play, the HUD up.
fn in_play(run: &Run, phase: &Phase) -> bool {
    run.begun && !matches!(phase, Phase::Menu | Phase::Lobby | Phase::GameOver)
}

/// The labels this frame, by key: every chest and live elite, and every empty post, in range.
fn wanted(
    run: &Run,
    hero: Vec3,
    marks: impl Iterator<Item = (Entity, Vec3, FieldMark, bool)>,
) -> Vec<(Key, Shown)> {
    let near = |p: Vec3| (p - hero).with_y(0.0).length() <= data::MARK_RANGE;
    let mut out = Vec::new();
    for (e, at, mark, dead) in marks {
        if dead || !near(at) {
            continue;
        }
        let shown = match mark {
            // A key in hand opens it for nothing.
            FieldMark::Chest if run.keys > 0 => Shown {
                at: at + Vec3::Y * 1.8,
                icon: data::CHEST_ICON,
                title: trf("Key ({} held)", &[&run.keys.to_string()]),
                line: tr("Stand here to open").to_string(),
                color: data::CHEST_COLOR,
            },
            FieldMark::Chest => {
                let cost = run.chest_cost;
                let afford = run.gold + 1e-3 >= cost as f32;
                Shown {
                    at: at + Vec3::Y * 1.8,
                    icon: data::CHEST_ICON,
                    title: trf("{} gold", &[&cost.to_string()]),
                    line: if afford {
                        tr("Stand here to open").to_string()
                    } else {
                        trf("You have {}", &[&run.gold.floor().to_string()])
                    },
                    color: if afford {
                        data::CHEST_COLOR
                    } else {
                        [0.62, 0.62, 0.62]
                    },
                }
            }
            FieldMark::FreeChest => Shown {
                at: at + Vec3::Y * 1.8,
                icon: data::CHEST_ICON,
                title: tr("Free").to_string(),
                line: tr("Stand here to open").to_string(),
                color: data::CHEST_COLOR,
            },
            FieldMark::Merchant => Shown {
                at: at + Vec3::Y * 2.6,
                icon: data::MERCHANT_ICON,
                title: tr("Shady Merchant").to_string(),
                line: tr("Stand near to trade").to_string(),
                color: data::MERCHANT_COLOR,
            },
            FieldMark::Elite(k) => {
                let kind = &data::ELITES[usize::from(k).min(data::ELITES.len() - 1)];
                Shown {
                    at: at + Vec3::Y * 4.2,
                    icon: kind.icon,
                    title: tr(kind.title).to_string(),
                    line: reward_text(kind.reward),
                    color: kind.color,
                }
            }
        };
        out.push((Key::Unit(e), shown));
    }
    for (i, post) in run.posts.iter().enumerate() {
        if post.wait <= 0.0 || !near(post.pos) {
            continue;
        }
        let kind = &data::ELITES[post.kind.min(data::ELITES.len() - 1)];
        out.push((
            Key::Post(i),
            Shown {
                at: post.pos + Vec3::Y * 2.0,
                icon: kind.icon,
                title: tr(kind.title).to_string(),
                line: trf("Back in {}s", &[&seconds_left(post.wait).to_string()]),
                color: dim(kind.color, 0.7),
            },
        ));
    }
    // Shrines waiting, or a challenge's elites still up; a spent one has nothing more to say.
    for (i, sh) in run.shrines.iter().enumerate() {
        if sh.state == ShrineState::Spent || !near(sh.pos) {
            continue;
        }
        let def = data::shrine(sh.kind);
        let line = match sh.state {
            ShrineState::Fighting => tr("Beat its elites").to_string(),
            _ if sh.charge > 0.0 => format!("{}  ·  {}%", tr(def.text), (sh.charge * 100.0) as u32),
            _ => tr(def.text).to_string(),
        };
        out.push((
            Key::Shrine(i),
            Shown {
                at: sh.pos + Vec3::Y * 3.0,
                icon: def.spell,
                title: tr(def.title).to_string(),
                line,
                color: def.color,
            },
        ));
    }
    out
}

fn spawn_label(commands: &mut Commands, font: &Handle<Font>, root: Entity, s: &Shown) -> Label {
    let shadow = TextShadow {
        offset: Vec2::splat(1.5),
        color: Color::BLACK,
    };
    let mut title = Entity::PLACEHOLDER;
    let mut line = Entity::PLACEHOLDER;
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(LABEL_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|l| {
            l.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: Val::Px(LABEL_ICON),
                        height: Val::Px(LABEL_ICON),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    ImageNode {
                        color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                        ..default()
                    },
                    SpellIcon(s.icon),
                ));
                title = row
                    .spawn((
                        Text::new(s.title.clone()),
                        TextFont {
                            font: font.clone(),
                            font_size: LABEL_TITLE,
                            ..default()
                        },
                        TextColor(srgb(s.color)),
                        shadow,
                    ))
                    .id();
            });
            line = l
                .spawn((
                    Text::new(s.line.clone()),
                    TextFont {
                        font: font.clone(),
                        font_size: LABEL_LINE,
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.92, 0.92)),
                    TextLayout::new_with_justify(Justify::Center),
                    shadow,
                ))
                .id();
        })
        .id();
    commands.entity(root).add_child(node);
    Label {
        root: node,
        title,
        line,
    }
}

/// The labels over the field: spawned as chests, elites and empty posts come in range, moved to
/// where they show on screen, and despawned once they are gone.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn place_labels(
    mut commands: Commands,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    font: Option<Res<HudFont>>,
    ui_scale: Res<UiScale>,
    roots: Query<Entity, With<HudRoot>>,
    hero: Query<&Transform, With<LocalHero>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    marks: Query<(Entity, &Transform, &FieldMark, Option<&ObjectStore>)>,
    mut nodes: Query<(&mut Node, &mut Visibility)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut shown: Local<HashMap<Key, Label>>,
) {
    let want = match hero.single() {
        Ok(h) if in_play(&run, phase.get()) => wanted(
            &run,
            h.translation,
            marks
                .iter()
                .map(|(e, tf, m, store)| (e, tf.translation, *m, store.is_some_and(is_dead))),
        ),
        _ => Vec::new(),
    };
    // Gone: its unit fell or left the range, its post came back, the run ended.
    shown.retain(|key, label| {
        let keep = want.iter().any(|(k, _)| k == key);
        if !keep {
            commands.entity(label.root).despawn();
        }
        keep
    });
    if want.is_empty() {
        return;
    }
    let (Some(font), Ok(root)) = (font, roots.single()) else {
        return;
    };
    let cam = cams.single().ok();
    let s = ui_scale.0.max(1e-3);
    for (key, w) in &want {
        let Some(label) = shown.get(key) else {
            // Placed from the next frame, once its nodes exist.
            let label = spawn_label(&mut commands, &font.0, root, w);
            shown.insert(*key, label);
            continue;
        };
        let at =
            cam.and_then(|(c, tf)| c.world_to_viewport(&GlobalTransform::from(*tf), w.at).ok());
        if let Ok((mut node, mut vis)) = nodes.get_mut(label.root) {
            match at {
                Some(p) => {
                    node.left = Val::Px(p.x / s - LABEL_W / 2.0);
                    node.top = Val::Px(p.y / s - LABEL_ICON - LABEL_LINE - 6.0);
                    *vis = Visibility::Inherited;
                }
                None => *vis = Visibility::Hidden,
            }
        }
        for (e, text, color) in [
            (label.title, &w.title, srgb(w.color)),
            (label.line, &w.line, Color::srgb(0.92, 0.92, 0.92)),
        ] {
            if let Ok((mut t, mut c)) = texts.get_mut(e) {
                if t.0 != *text {
                    t.0.clone_from(text);
                }
                if c.0 != color {
                    c.0 = color;
                }
            }
        }
    }
}

/// A ring on the ground around `center`, `radius` yards out; `dash` leaves every other segment
/// out, and `fill` (0..1) draws only that much of it, clockwise from north.
fn ground_ring(
    gizmos: &mut Gizmos,
    world: &WorldPoint,
    center: Vec3,
    radius: f32,
    color: Color,
    dash: bool,
    fill: f32,
) {
    let point = |i: usize| {
        let a = i as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let mut p = center + Vec3::new(a.sin(), 0.0, -a.cos()) * radius;
        p.y = world.terrain_height_under(p).unwrap_or(center.y) + RING_LIFT;
        p
    };
    let last = (fill.clamp(0.0, 1.0) * RING_SEGMENTS as f32).round() as usize;
    for i in 0..last {
        if dash && i % 2 == 1 {
            continue;
        }
        gizmos.line(point(i), point(i + 1), color);
    }
}

/// The rings on the ground: a chest's reach (gold when the hero can pay, grey when not), each
/// elite's in its reward's colour, and a fallen elite's post dashed, filling as it comes back.
pub(super) fn draw_rings(
    mut gizmos: Gizmos,
    world: WorldPoint,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    hero: Query<&Transform, With<LocalHero>>,
    marks: Query<(&Transform, &FieldMark, Option<&ObjectStore>)>,
) {
    if !in_play(&run, phase.get()) {
        return;
    }
    let Ok(hero) = hero.single() else { return };
    let near = |p: Vec3| (p - hero.translation).with_y(0.0).length() <= data::MARK_RANGE;
    for (tf, mark, store) in &marks {
        let at = tf.translation;
        if store.is_some_and(is_dead) || !near(at) {
            continue;
        }
        match mark {
            FieldMark::Chest | FieldMark::FreeChest => {
                let afford = *mark == FieldMark::FreeChest
                    || run.keys > 0
                    || run.gold + 1e-3 >= run.chest_cost as f32;
                let color = if afford {
                    srgb(data::CHEST_COLOR)
                } else {
                    Color::srgb(0.5, 0.5, 0.5)
                };
                ground_ring(
                    &mut gizmos,
                    &world,
                    at,
                    data::CHEST_REACH,
                    color,
                    false,
                    1.0,
                );
            }
            FieldMark::Merchant => {
                ground_ring(
                    &mut gizmos,
                    &world,
                    at,
                    data::MERCHANT_REACH,
                    srgb(data::MERCHANT_COLOR),
                    true,
                    1.0,
                );
            }
            FieldMark::Elite(k) => {
                let kind = &data::ELITES[usize::from(*k).min(data::ELITES.len() - 1)];
                let r = ELITE_RING * tf.scale.x.max(1.0);
                ground_ring(&mut gizmos, &world, at, r, srgb(kind.color), false, 1.0);
                ground_ring(
                    &mut gizmos,
                    &world,
                    at,
                    r * 1.15,
                    srgb(kind.color),
                    true,
                    1.0,
                );
            }
        }
    }
    for post in &run.posts {
        if post.wait <= 0.0 || !near(post.pos) {
            continue;
        }
        let c = data::ELITES[post.kind.min(data::ELITES.len() - 1)].color;
        ground_ring(
            &mut gizmos,
            &world,
            post.pos,
            ELITE_RING * 1.6,
            srgb(dim(c, 0.6)),
            true,
            1.0,
        );
        let back = returning(post.kind, post.wait);
        ground_ring(
            &mut gizmos,
            &world,
            post.pos,
            ELITE_RING * 1.3,
            srgb(c),
            false,
            back,
        );
    } // A shrine's reach, filling as it charges; a challenge's dashed while its elites are up.
    for sh in &run.shrines {
        if sh.state == ShrineState::Spent || !near(sh.pos) {
            continue;
        }
        let c = data::shrine(sh.kind).color;
        let fighting = sh.state == ShrineState::Fighting;
        ground_ring(
            &mut gizmos,
            &world,
            sh.pos,
            data::SHRINE_RADIUS,
            srgb(dim(c, 0.7)),
            fighting,
            1.0,
        );
        if !fighting && sh.charge > 0.0 {
            ground_ring(
                &mut gizmos,
                &world,
                sh.pos,
                data::SHRINE_RADIUS * 0.8,
                srgb(c),
                false,
                sh.charge,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::treasure::PostView;

    fn run_with(gold: f32, cost: u32) -> Run {
        let mut run = Run {
            chest_cost: cost,
            ..Run::default()
        };
        run.me.gold = gold;
        run
    }

    #[test]
    fn a_chest_reads_its_price_and_whether_the_hero_can_pay() {
        let chest = Entity::from_raw_u32(7).unwrap();
        let rich = wanted(
            &run_with(50.0, 20),
            Vec3::ZERO,
            [(chest, Vec3::X * 10.0, FieldMark::Chest, false)].into_iter(),
        );
        assert_eq!(rich.len(), 1);
        assert_eq!(rich[0].0, Key::Unit(chest));
        assert!(rich[0].1.title.contains("20"));
        assert_eq!(rich[0].1.color, data::CHEST_COLOR);
        let poor = wanted(
            &run_with(5.0, 20),
            Vec3::ZERO,
            [(chest, Vec3::X * 10.0, FieldMark::Chest, false)].into_iter(),
        );
        assert_ne!(poor[0].1.color, data::CHEST_COLOR);
        assert!(poor[0].1.line.contains('5'), "{}", poor[0].1.line);
    }

    #[test]
    fn an_elite_shows_its_reward_and_a_fallen_one_its_countdown() {
        let elite = Entity::from_raw_u32(9).unwrap();
        let mut run = run_with(0.0, 20);
        run.posts = vec![
            PostView {
                kind: 0,
                pos: Vec3::Z * 20.0,
                wait: 0.0,
            },
            PostView {
                kind: 3,
                pos: Vec3::Z * -20.0,
                wait: 41.2,
            },
        ];
        let got = wanted(
            &run,
            Vec3::ZERO,
            [(elite, Vec3::Z * 20.0, FieldMark::Elite(0), false)].into_iter(),
        );
        // The standing post's elite is labelled by its unit, the empty post by its countdown.
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1.line, reward_text(data::ELITES[0].reward));
        assert_eq!(got[0].1.icon, data::ELITES[0].icon);
        assert_eq!(got[1].0, Key::Post(1));
        assert!(got[1].1.line.contains("42"), "{}", got[1].1.line);
    }

    #[test]
    fn out_of_range_or_dead_marks_have_no_label() {
        let e = Entity::from_raw_u32(3).unwrap();
        let far = Vec3::X * (data::MARK_RANGE + 1.0);
        let got = wanted(
            &run_with(0.0, 20),
            Vec3::ZERO,
            [
                (e, far, FieldMark::Chest, false),
                (e, Vec3::X, FieldMark::Elite(1), true),
            ]
            .into_iter(),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn a_post_fills_as_its_elite_comes_back() {
        let total = data::ELITES[1].respawn;
        assert_eq!(returning(1, total), 0.0);
        assert!((returning(1, total / 2.0) - 0.5).abs() < 1e-5);
        assert_eq!(returning(1, 0.0), 1.0);
        assert_eq!(seconds_left(0.2), 1);
        assert_eq!(seconds_left(-3.0), 0);
    }

    #[test]
    fn every_reward_has_a_line() {
        for e in data::ELITES {
            assert!(!reward_text(e.reward).is_empty());
        }
    }
}
