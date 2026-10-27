//! The run's own messages, carried inside the lobby relay. The host plays the run and streams it
//! the way a WoW server streams a world — units with their update fields, their motion, spell
//! casts and impacts, swings and combat text — so a client's engine builds and animates them
//! exactly as it does streamed units. Clients send back their own hero's motion, card picks and
//! mobility casts.

use crate::protocol::Slot;
use crate::wire::{Reader, Wire, WireError, Writer};

/// A unit's GUID, as the host minted it.
pub type Guid = u64;

/// What a replicated object is to the game, beyond its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Enemy,
    /// A player's hero, by seat.
    Hero(Slot),
    Totem,
    /// An experience wisp or a health pickup: its model's loops are held still.
    Pickup,
    /// A ground spell's dynamic object.
    Area,
    /// A chest of the run's field, opened with the run's gold.
    Chest,
    /// An elite holding a post of the field, by kind (`data::ELITES`).
    Elite(u8),
    /// A Shrine of Challenge's prize: a chest that opens for nothing.
    FreeChest,
    /// The shady merchant.
    Merchant,
}

/// A portal of the run's stage: the boss portal (`0`), which calls the stage's final boss, or the
/// portal onward (`1`), to the next stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gate {
    pub kind: u8,
    pub pos: [f32; 3],
    /// How charged, 0 to 1.
    pub charge: f32,
    /// Whether a hero may wake it yet.
    pub open: bool,
}

/// One of the merchant's wares: a run item by key, its price, and whether it is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ware {
    pub item: String,
    pub price: u32,
    pub sold: bool,
}

/// A bonus as it travels: `(stat, 0 flat or 1 percent, value × 10 000)`, as a loadout's.
pub type WireBonus = (u8, u8, i32);

/// A blessing a Shrine of Power offers: its bonus and its rarity (`0` Common to `4` Legendary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boon {
    pub bonus: WireBonus,
    pub rarity: u8,
}

/// One shrine of the field, as the host tends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shrine {
    /// Its kind (`data::SHRINES`).
    pub kind: u8,
    pub pos: [f32; 3],
    /// How charged, 0 to 1.
    pub charge: f32,
    /// `0` waiting, `1` its elites up, `2` spent.
    pub state: u8,
}

/// One elite's post of the run's field, as the host tends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Post {
    /// The elite's kind (`data::ELITES`).
    pub kind: u8,
    pub pos: [f32; 3],
    /// Seconds until its elite comes back; 0 while it stands.
    pub wait: f32,
}

/// The engine's object kind, as `NetEntity` holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Unit,
    GameObject,
    DynamicObject,
    /// A hero drawn as a player character, dressed from its fields.
    Player,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
    pub guid: Guid,
    pub kind: ObjectKind,
    pub role: Role,
    pub display: Option<u32>,
    pub scale: f32,
    /// The object's update fields, `(index, value)`.
    pub fields: Vec<(u16, u32)>,
    pub pos: [f32; 3],
    pub yaw: f32,
}

/// Where a unit is and how it moves, for the animation driver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub guid: Guid,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub speed: f32,
    /// `MovementState::flags`.
    pub flags: u32,
    pub vertical_speed: f32,
}

/// An owned ability as the HUD shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityView {
    pub key: String,
    /// Its rank: no cap.
    pub level: u16,
    /// Seconds of cooldown left.
    pub cd: f32,
    /// What its cards gave it, summed: the damage share added, the cooldown share left, the
    /// targets added, the area and duration shares added.
    pub damage: f32,
    pub cooldown: f32,
    pub targets: u8,
    pub area: f32,
    pub duration: f32,
}

/// One hero's numbers, sent to its own player.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Vitals {
    pub level: u32,
    pub xp: f32,
    pub hp: f32,
    pub abilities: Vec<AbilityView>,
    /// Each passive's key, rank, and the ranks' worth its bonus grows by.
    pub passives: Vec<(String, u8, f32)>,
    pub shield: f32,
    pub shield_t: f32,
    pub immune_t: f32,
    pub thorns_t: f32,
    pub heal_t: f32,
    pub pending_levels: u32,
    /// Seconds since the hero fell.
    pub dead_t: Option<f32>,
    pub damage_done: f64,
    pub kills: u32,
    /// Card picks the host has applied for this hero, so the client knows which of its own picks
    /// these numbers already include.
    pub picks: u32,
    /// Seconds an ally has stood over this fallen hero, toward the revive.
    pub revive_t: f32,
    /// The items this hero has found this run, by name.
    pub loot: Vec<String>,
    /// The run's gold in hand.
    pub gold: f32,
    /// The run's items, by key, and their stacks.
    pub run_items: Vec<(String, u8)>,
    /// The level-up cards the host dealt this hero, which only they may be picked from.
    pub offer: Vec<Card>,
    /// What the hero has left to reroll the cards, banish one, or skip a level.
    pub rerolls: u8,
    pub banishes: u8,
    /// The blessings a Shrine of Power offers this hero, which only one may be taken from.
    pub shrine_offer: Vec<Boon>,
    /// What the shrines gave this hero for the run.
    pub boons: Vec<WireBonus>,
    /// Keys in hand: each opens a chest for nothing.
    pub keys: u8,
    /// The damage done this run by spell (`Spell.dbc` row, damage), for the damage meter.
    pub damage_by: Vec<(u32, f32)>,
}

/// A hero in the party frames.
#[derive(Debug, Clone, PartialEq)]
pub struct PartyEntry {
    pub slot: Slot,
    pub guid: Guid,
    pub level: u32,
    pub hp: f32,
    pub max_hp: f32,
    pub dead: bool,
}

/// A level-up card, by key rather than index, so it means the same on both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    NewAbility(String),
    Upgrade(String),
    NewPassive(String),
    UpPassive(String),
    Heal,
}

/// A dealt card: what it gives, the rarity it was drawn at (`0` Common to `4` Legendary), and
/// the stats of its spell it grows, `(kind, value)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub choice: Choice,
    pub rarity: u8,
    pub perks: Vec<(u8, f32)>,
}

/// What a player does at the level-up cards, for the host to check against the hero's offer.
#[derive(Debug, Clone, PartialEq)]
pub enum CardAction {
    /// Take a card of the offer.
    Pick(Card),
    /// Deal the offer again, spending a reroll.
    Reroll,
    /// Take a card out of the hero's deals for the rest of the run, spending a banish.
    Banish(Card),
    /// Let a level go by without a card, spending a skip.
    Skip,
}

/// What a mobility cast does to the horde, for the host to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MobilityEffect {
    None,
    /// Charge: stun one enemy.
    Stun {
        target: Guid,
        secs: f32,
    },
    /// Howl: every non-boss within `radius` of `at` flees.
    Fear {
        at: [f32; 3],
        radius: f32,
        secs: f32,
    },
}

/// How a run ended, per hero.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub slot: Slot,
    pub level: u32,
    pub kills: u32,
    pub damage_done: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Game {
    // --- host → clients ---
    Spawn(Spawn),
    /// Changed update fields.
    Fields {
        guid: Guid,
        changes: Vec<(u16, u32)>,
    },
    /// Every moving unit, at the host's run clock `t`.
    Motions {
        t: f32,
        list: Vec<Motion>,
    },
    /// The object is gone; `fade` plays the death fade first.
    Despawn {
        guid: Guid,
        fade: bool,
    },
    Cast {
        caster: Guid,
        spell: u32,
        hits: Vec<Guid>,
        dest: Option<[f32; 3]>,
    },
    Impact {
        target: Guid,
        spell: u32,
    },
    Swing {
        attacker: Guid,
        victim: Guid,
        damage: u32,
    },
    Number {
        anchor: Guid,
        amount: f32,
        crit: bool,
        color: u32,
    },
    /// The run's shared clock and announcements.
    Run {
        elapsed: f32,
        bosses: u32,
        /// Still waiting for every player to load the zone.
        waiting: bool,
        banner: Option<(String, f32)>,
        /// What the next chest costs.
        chest_cost: u32,
        /// The field's elite posts.
        posts: Vec<Post>,
        /// The field's shrines, and the Greed and Curses woken.
        shrines: Vec<Shrine>,
        greed: u8,
        curse: u8,
        /// The merchant's wares.
        shop: Vec<Ware>,
        /// The stage in play (from 0), the zone it stands in, its portals, and the final swarm's
        /// seconds left (negative while it does not run).
        stage: u8,
        zone: u8,
        gates: Vec<Gate>,
        swarm: f32,
    },
    /// Your hero's numbers.
    Me(Vitals),
    Party(Vec<PartyEntry>),
    Outcome {
        won: bool,
        scores: Vec<Score>,
    },

    // --- clients → host ---
    /// The zone is loaded and the client is ready to play.
    Loaded,
    /// The client's own hero, which it moves itself.
    Input {
        /// The client's clock when it sent this, for the host to draw the hero between two
        /// positions on it.
        t: f32,
        motion: Motion,
        /// The class mobility's running haste and vanish, which the client owns.
        haste: f32,
        haste_t: f32,
        vanish_t: f32,
    },
    /// A move at the level-up cards.
    Card(CardAction),
    /// The blessing taken from a Shrine of Power's offer, by its bonus.
    ShrineChoice(WireBonus),
    /// A ware bought from the merchant, by its place and its item.
    Buy {
        slot: u8,
        item: String,
    },
    Mobility {
        spell: u32,
        effect: MobilityEffect,
    },
    /// The client came back to the run with nothing: the host sends it the whole world again.
    Resync,
}

/// A tick's messages in one relay frame: each one length-prefixed, so the server forwards one
/// frame per tick rather than one per spell.
pub fn encode_batch(msgs: &[Game]) -> Vec<u8> {
    let mut w = Writer::new();
    for m in msgs {
        let mut inner = Writer::new();
        m.put(&mut inner);
        w.bytes(&inner.buf);
    }
    w.finish()
}

/// The messages of a batch, in order. A message that does not decode is skipped: one bad message
/// does not cost the rest of the tick.
pub fn decode_batch(buf: &[u8]) -> Vec<Game> {
    let mut r = Reader::new(buf);
    let mut out = Vec::new();
    while r.end().is_err() {
        match r.bytes() {
            Ok(b) => {
                if let Ok(m) = Game::decode(&b) {
                    out.push(m);
                }
            }
            Err(_) => break,
        }
    }
    out
}

// --- codec ---------------------------------------------------------------------------------

impl Wire for Role {
    fn put(&self, w: &mut Writer) {
        match self {
            Role::Enemy => w.u8(0),
            Role::Hero(s) => w.u8(1).u8(*s),
            Role::Totem => w.u8(2),
            Role::Pickup => w.u8(3),
            Role::Area => w.u8(4),
            Role::Chest => w.u8(5),
            Role::Elite(k) => w.u8(6).u8(*k),
            Role::FreeChest => w.u8(7),
            Role::Merchant => w.u8(8),
        };
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => Role::Enemy,
            1 => Role::Hero(r.u8()?),
            2 => Role::Totem,
            3 => Role::Pickup,
            4 => Role::Area,
            5 => Role::Chest,
            6 => Role::Elite(r.u8()?),
            7 => Role::FreeChest,
            8 => Role::Merchant,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

impl Wire for ObjectKind {
    fn put(&self, w: &mut Writer) {
        w.u8(match self {
            ObjectKind::Unit => 0,
            ObjectKind::GameObject => 1,
            ObjectKind::DynamicObject => 2,
            ObjectKind::Player => 3,
        });
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => ObjectKind::Unit,
            1 => ObjectKind::GameObject,
            2 => ObjectKind::DynamicObject,
            3 => ObjectKind::Player,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

fn put_pairs(w: &mut Writer, pairs: &[(u16, u32)]) {
    w.len(pairs.len());
    for &(i, v) in pairs.iter().take(u16::MAX as usize) {
        w.u16(i).u32(v);
    }
}

fn get_pairs(r: &mut Reader) -> Result<Vec<(u16, u32)>, WireError> {
    let n = r.len(6)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push((r.u16()?, r.u32()?));
    }
    Ok(v)
}

impl Wire for Spawn {
    fn put(&self, w: &mut Writer) {
        w.u64(self.guid);
        self.kind.put(w);
        self.role.put(w);
        w.opt(self.display, |w, d| {
            w.u32(d);
        })
        .f32(self.scale);
        put_pairs(w, &self.fields);
        w.vec3(self.pos).f32(self.yaw);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            guid: r.u64()?,
            kind: ObjectKind::get(r)?,
            role: Role::get(r)?,
            display: r.opt(|r| r.u32())?,
            scale: r.f32()?,
            fields: get_pairs(r)?,
            pos: r.vec3()?,
            yaw: r.f32()?,
        })
    }
}

impl Wire for Motion {
    fn put(&self, w: &mut Writer) {
        w.u64(self.guid)
            .vec3(self.pos)
            .f32(self.yaw)
            .f32(self.speed)
            .u32(self.flags)
            .f32(self.vertical_speed);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            guid: r.u64()?,
            pos: r.vec3()?,
            yaw: r.f32()?,
            speed: r.f32()?,
            flags: r.u32()?,
            vertical_speed: r.f32()?,
        })
    }
}

impl Wire for Vitals {
    fn put(&self, w: &mut Writer) {
        w.u32(self.level)
            .f32(self.xp)
            .f32(self.hp)
            .len(self.abilities.len());
        for a in &self.abilities {
            w.str(&a.key)
                .u16(a.level)
                .f32(a.cd)
                .f32(a.damage)
                .f32(a.cooldown)
                .u8(a.targets)
                .f32(a.area)
                .f32(a.duration);
        }
        w.len(self.passives.len());
        for (k, l, p) in &self.passives {
            w.str(k).u8(*l).f32(*p);
        }
        w.f32(self.shield)
            .f32(self.shield_t)
            .f32(self.immune_t)
            .f32(self.thorns_t)
            .f32(self.heal_t)
            .u32(self.pending_levels)
            .opt(self.dead_t, |w, t| {
                w.f32(t);
            })
            .u64(self.damage_done.max(0.0).round() as u64)
            .u32(self.kills)
            .u32(self.picks)
            .f32(self.revive_t)
            .len(self.loot.len());
        for name in &self.loot {
            w.str(name);
        }
        w.f32(self.gold).len(self.run_items.len());
        for (k, n) in &self.run_items {
            w.str(k).u8(*n);
        }
        w.len(self.offer.len());
        for c in &self.offer {
            c.put(w);
        }
        w.u8(self.rerolls).u8(self.banishes);
        w.len(self.shrine_offer.len());
        for b in &self.shrine_offer {
            put_bonus(w, b.bonus);
            w.u8(b.rarity);
        }
        w.len(self.boons.len());
        for b in &self.boons {
            put_bonus(w, *b);
        }
        w.u8(self.keys).len(self.damage_by.len());
        for (s, d) in &self.damage_by {
            w.u32(*s).f32(*d);
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        let level = r.u32()?;
        let xp = r.f32()?;
        let hp = r.f32()?;
        let n = r.len(26)?;
        let mut abilities = Vec::with_capacity(n);
        for _ in 0..n {
            abilities.push(AbilityView {
                key: r.str()?,
                level: r.u16()?,
                cd: r.f32()?,
                damage: r.f32()?,
                cooldown: r.f32()?,
                targets: r.u8()?,
                area: r.f32()?,
                duration: r.f32()?,
            });
        }
        let n = r.len(7)?;
        let mut passives = Vec::with_capacity(n);
        for _ in 0..n {
            passives.push((r.str()?, r.u8()?, r.f32()?));
        }
        Ok(Self {
            level,
            xp,
            hp,
            abilities,
            passives,
            shield: r.f32()?,
            shield_t: r.f32()?,
            immune_t: r.f32()?,
            thorns_t: r.f32()?,
            heal_t: r.f32()?,
            pending_levels: r.u32()?,
            dead_t: r.opt(|r| r.f32())?,
            damage_done: r.u64()? as f64,
            kills: r.u32()?,
            picks: r.u32()?,
            revive_t: r.f32()?,
            loot: {
                let n = r.len(1)?;
                let mut loot = Vec::with_capacity(n);
                for _ in 0..n {
                    loot.push(r.str()?);
                }
                loot
            },
            gold: r.f32()?,
            run_items: {
                let n = r.len(3)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push((r.str()?, r.u8()?));
                }
                out
            },
            offer: {
                let n = r.len(2)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(Card::get(r)?);
                }
                out
            },
            rerolls: r.u8()?,
            banishes: r.u8()?,
            shrine_offer: {
                let n = r.len(7)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(Boon {
                        bonus: get_bonus(r)?,
                        rarity: r.u8()?,
                    });
                }
                out
            },
            boons: {
                let n = r.len(6)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(get_bonus(r)?);
                }
                out
            },
            keys: r.u8()?,
            damage_by: {
                let n = r.len(8)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push((r.u32()?, r.f32()?));
                }
                out
            },
        })
    }
}

fn put_bonus(w: &mut Writer, (stat, kind, v): WireBonus) {
    w.u8(stat).u8(kind).u32(v as u32);
}

fn get_bonus(r: &mut Reader) -> Result<WireBonus, WireError> {
    Ok((r.u8()?, r.u8()?, r.u32()? as i32))
}

impl Wire for Card {
    fn put(&self, w: &mut Writer) {
        self.choice.put(w);
        w.u8(self.rarity).len(self.perks.len());
        for (k, v) in &self.perks {
            w.u8(*k).f32(*v);
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        let choice = Choice::get(r)?;
        let rarity = r.u8()?;
        let n = r.len(5)?;
        let mut perks = Vec::with_capacity(n);
        for _ in 0..n {
            perks.push((r.u8()?, r.f32()?));
        }
        Ok(Self {
            choice,
            rarity,
            perks,
        })
    }
}

impl Wire for CardAction {
    fn put(&self, w: &mut Writer) {
        match self {
            CardAction::Pick(c) => {
                w.u8(0);
                c.put(w);
            }
            CardAction::Reroll => {
                w.u8(1);
            }
            CardAction::Banish(c) => {
                w.u8(2);
                c.put(w);
            }
            CardAction::Skip => {
                w.u8(3);
            }
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => CardAction::Pick(Card::get(r)?),
            1 => CardAction::Reroll,
            2 => CardAction::Banish(Card::get(r)?),
            3 => CardAction::Skip,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

impl Wire for Choice {
    fn put(&self, w: &mut Writer) {
        match self {
            Choice::NewAbility(k) => w.u8(0).str(k),
            Choice::Upgrade(k) => w.u8(1).str(k),
            Choice::NewPassive(k) => w.u8(2).str(k),
            Choice::UpPassive(k) => w.u8(3).str(k),
            Choice::Heal => w.u8(4),
        };
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => Choice::NewAbility(r.str()?),
            1 => Choice::Upgrade(r.str()?),
            2 => Choice::NewPassive(r.str()?),
            3 => Choice::UpPassive(r.str()?),
            4 => Choice::Heal,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

impl Wire for MobilityEffect {
    fn put(&self, w: &mut Writer) {
        match *self {
            MobilityEffect::None => {
                w.u8(0);
            }
            MobilityEffect::Stun { target, secs } => {
                w.u8(1).u64(target).f32(secs);
            }
            MobilityEffect::Fear { at, radius, secs } => {
                w.u8(2).vec3(at).f32(radius).f32(secs);
            }
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => MobilityEffect::None,
            1 => MobilityEffect::Stun {
                target: r.u64()?,
                secs: r.f32()?,
            },
            2 => MobilityEffect::Fear {
                at: r.vec3()?,
                radius: r.f32()?,
                secs: r.f32()?,
            },
            t => return Err(WireError::BadTag(t)),
        })
    }
}

impl Wire for Game {
    fn put(&self, w: &mut Writer) {
        match self {
            Game::Spawn(s) => {
                w.u8(0);
                s.put(w);
            }
            Game::Fields { guid, changes } => {
                w.u8(1).u64(*guid);
                put_pairs(w, changes);
            }
            Game::Motions { t, list } => {
                w.u8(2).f32(*t).len(list.len());
                for m in list {
                    m.put(w);
                }
            }
            Game::Despawn { guid, fade } => {
                w.u8(3).u64(*guid).bool(*fade);
            }
            Game::Cast {
                caster,
                spell,
                hits,
                dest,
            } => {
                w.u8(4).u64(*caster).u32(*spell).len(hits.len());
                for h in hits {
                    w.u64(*h);
                }
                w.opt(*dest, |w, d| {
                    w.vec3(d);
                });
            }
            Game::Impact { target, spell } => {
                w.u8(5).u64(*target).u32(*spell);
            }
            Game::Swing {
                attacker,
                victim,
                damage,
            } => {
                w.u8(6).u64(*attacker).u64(*victim).u32(*damage);
            }
            Game::Number {
                anchor,
                amount,
                crit,
                color,
            } => {
                w.u8(7).u64(*anchor).f32(*amount).bool(*crit).u32(*color);
            }
            Game::Run {
                elapsed,
                bosses,
                waiting,
                banner,
                chest_cost,
                posts,
                shrines,
                greed,
                curse,
                shop,
                stage,
                zone,
                gates,
                swarm,
            } => {
                w.u8(8).f32(*elapsed).u32(*bosses).bool(*waiting).opt(
                    banner.as_ref(),
                    |w, (s, t)| {
                        w.str(s).f32(*t);
                    },
                );
                w.u32(*chest_cost).len(posts.len());
                for p in posts {
                    w.u8(p.kind).vec3(p.pos).f32(p.wait);
                }
                w.len(shrines.len());
                for s in shrines {
                    w.u8(s.kind).vec3(s.pos).f32(s.charge).u8(s.state);
                }
                w.u8(*greed).u8(*curse).len(shop.len());
                for ware in shop {
                    w.str(&ware.item).u32(ware.price).bool(ware.sold);
                }
                w.u8(*stage).u8(*zone).len(gates.len());
                for g in gates {
                    w.u8(g.kind).vec3(g.pos).f32(g.charge).bool(g.open);
                }
                w.f32(*swarm);
            }
            Game::Me(v) => {
                w.u8(9);
                v.put(w);
            }
            Game::Party(list) => {
                w.u8(10).len(list.len());
                for p in list {
                    w.u8(p.slot)
                        .u64(p.guid)
                        .u32(p.level)
                        .f32(p.hp)
                        .f32(p.max_hp)
                        .bool(p.dead);
                }
            }
            Game::Outcome { won, scores } => {
                w.u8(11).bool(*won).len(scores.len());
                for s in scores {
                    w.u8(s.slot)
                        .u32(s.level)
                        .u32(s.kills)
                        .u64(s.damage_done.max(0.0).round() as u64);
                }
            }
            Game::Loaded => {
                w.u8(12);
            }
            Game::Input {
                t,
                motion,
                haste,
                haste_t,
                vanish_t,
            } => {
                w.u8(13).f32(*t);
                motion.put(w);
                w.f32(*haste).f32(*haste_t).f32(*vanish_t);
            }
            Game::Card(a) => {
                w.u8(14);
                a.put(w);
            }
            Game::ShrineChoice(b) => {
                w.u8(16);
                put_bonus(w, *b);
            }
            Game::Buy { slot, item } => {
                w.u8(17).u8(*slot).str(item);
            }
            Game::Mobility { spell, effect } => {
                w.u8(15).u32(*spell);
                effect.put(w);
            }
            Game::Resync => {
                w.u8(18);
            }
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => Game::Spawn(Spawn::get(r)?),
            1 => Game::Fields {
                guid: r.u64()?,
                changes: get_pairs(r)?,
            },
            2 => {
                let t = r.f32()?;
                let n = r.len(36)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push(Motion::get(r)?);
                }
                Game::Motions { t, list }
            }
            3 => Game::Despawn {
                guid: r.u64()?,
                fade: r.bool()?,
            },
            4 => {
                let caster = r.u64()?;
                let spell = r.u32()?;
                let n = r.len(8)?;
                let mut hits = Vec::with_capacity(n);
                for _ in 0..n {
                    hits.push(r.u64()?);
                }
                Game::Cast {
                    caster,
                    spell,
                    hits,
                    dest: r.opt(|r| r.vec3())?,
                }
            }
            5 => Game::Impact {
                target: r.u64()?,
                spell: r.u32()?,
            },
            6 => Game::Swing {
                attacker: r.u64()?,
                victim: r.u64()?,
                damage: r.u32()?,
            },
            7 => Game::Number {
                anchor: r.u64()?,
                amount: r.f32()?,
                crit: r.bool()?,
                color: r.u32()?,
            },
            8 => Game::Run {
                elapsed: r.f32()?,
                bosses: r.u32()?,
                waiting: r.bool()?,
                banner: r.opt(|r| Ok((r.str()?, r.f32()?)))?,
                chest_cost: r.u32()?,
                posts: {
                    let n = r.len(17)?;
                    let mut posts = Vec::with_capacity(n);
                    for _ in 0..n {
                        posts.push(Post {
                            kind: r.u8()?,
                            pos: r.vec3()?,
                            wait: r.f32()?,
                        });
                    }
                    posts
                },
                shrines: {
                    let n = r.len(18)?;
                    let mut out = Vec::with_capacity(n);
                    for _ in 0..n {
                        out.push(Shrine {
                            kind: r.u8()?,
                            pos: r.vec3()?,
                            charge: r.f32()?,
                            state: r.u8()?,
                        });
                    }
                    out
                },
                greed: r.u8()?,
                curse: r.u8()?,
                shop: {
                    let n = r.len(7)?;
                    let mut out = Vec::with_capacity(n);
                    for _ in 0..n {
                        out.push(Ware {
                            item: r.str()?,
                            price: r.u32()?,
                            sold: r.bool()?,
                        });
                    }
                    out
                },
                stage: r.u8()?,
                zone: r.u8()?,
                gates: {
                    let n = r.len(18)?;
                    let mut out = Vec::with_capacity(n);
                    for _ in 0..n {
                        out.push(Gate {
                            kind: r.u8()?,
                            pos: r.vec3()?,
                            charge: r.f32()?,
                            open: r.bool()?,
                        });
                    }
                    out
                },
                swarm: r.f32()?,
            },
            9 => Game::Me(Vitals::get(r)?),
            10 => {
                let n = r.len(22)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push(PartyEntry {
                        slot: r.u8()?,
                        guid: r.u64()?,
                        level: r.u32()?,
                        hp: r.f32()?,
                        max_hp: r.f32()?,
                        dead: r.bool()?,
                    });
                }
                Game::Party(list)
            }
            11 => {
                let won = r.bool()?;
                let n = r.len(17)?;
                let mut scores = Vec::with_capacity(n);
                for _ in 0..n {
                    scores.push(Score {
                        slot: r.u8()?,
                        level: r.u32()?,
                        kills: r.u32()?,
                        damage_done: r.u64()? as f64,
                    });
                }
                Game::Outcome { won, scores }
            }
            12 => Game::Loaded,
            13 => Game::Input {
                t: r.f32()?,
                motion: Motion::get(r)?,
                haste: r.f32()?,
                haste_t: r.f32()?,
                vanish_t: r.f32()?,
            },
            14 => Game::Card(CardAction::get(r)?),
            16 => Game::ShrineChoice(get_bonus(r)?),
            17 => Game::Buy {
                slot: r.u8()?,
                item: r.str()?,
            },
            15 => Game::Mobility {
                spell: r.u32()?,
                effect: MobilityEffect::get(r)?,
            },
            18 => Game::Resync,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn motion() -> Motion {
        Motion {
            guid: 0xF130_0000_0000_0042,
            pos: [1.0, 2.0, -3.5],
            yaw: 0.25,
            speed: 7.0,
            flags: 1,
            vertical_speed: -2.0,
        }
    }

    /// One of every game message, for the round trip and the fuzzer.
    pub(crate) fn samples() -> Vec<Game> {
        vec![
            Game::Spawn(Spawn {
                guid: 7,
                kind: ObjectKind::Unit,
                role: Role::Hero(2),
                display: Some(1234),
                scale: 1.5,
                fields: vec![(22, 100), (28, 100)],
                pos: [1.0, 2.0, 3.0],
                yaw: 1.0,
            }),
            Game::Spawn(Spawn {
                guid: 8,
                kind: ObjectKind::GameObject,
                role: Role::Pickup,
                display: None,
                scale: 1.0,
                fields: vec![],
                pos: [0.0; 3],
                yaw: 0.0,
            }),
            Game::Spawn(Spawn {
                guid: 9,
                kind: ObjectKind::GameObject,
                role: Role::Chest,
                display: Some(259),
                scale: 1.2,
                fields: vec![],
                pos: [5.0, 0.0, -5.0],
                yaw: 0.0,
            }),
            Game::Spawn(Spawn {
                guid: 10,
                kind: ObjectKind::Unit,
                role: Role::Elite(2),
                display: Some(321),
                scale: 1.6,
                fields: vec![(22, 900)],
                pos: [9.0, 1.0, 9.0],
                yaw: 0.5,
            }),
            Game::Spawn(Spawn {
                guid: 11,
                kind: ObjectKind::Unit,
                role: Role::FreeChest,
                display: Some(321),
                scale: 1.6,
                fields: vec![(22, 900)],
                pos: [9.0, 1.0, 9.0],
                yaw: 0.5,
            }),
            Game::Spawn(Spawn {
                guid: 12,
                kind: ObjectKind::Unit,
                role: Role::Merchant,
                display: Some(321),
                scale: 1.6,
                fields: vec![(22, 900)],
                pos: [9.0, 1.0, 9.0],
                yaw: 0.5,
            }),
            Game::Fields {
                guid: 7,
                changes: vec![(22, 0)],
            },
            Game::Motions {
                t: 12.5,
                list: vec![motion(), motion()],
            },
            Game::Despawn {
                guid: 9,
                fade: true,
            },
            Game::Cast {
                caster: 1,
                spell: 133,
                hits: vec![2, 3],
                dest: Some([1.0, 1.0, 1.0]),
            },
            Game::Impact {
                target: 2,
                spell: 403,
            },
            Game::Swing {
                attacker: 3,
                victim: 1,
                damage: 12,
            },
            Game::Number {
                anchor: 2,
                amount: 41.0,
                crit: true,
                color: 0xFFFF_DE00,
            },
            Game::Run {
                elapsed: 61.0,
                bosses: 1,
                waiting: false,
                banner: Some(("Echeyakee approaches!".into(), 4.0)),
                chest_cost: 30,
                posts: vec![
                    Post {
                        kind: 0,
                        pos: [1.0, 2.0, 3.0],
                        wait: 0.0,
                    },
                    Post {
                        kind: 3,
                        pos: [-4.0, 0.5, 9.0],
                        wait: 41.5,
                    },
                ],
                shrines: vec![Shrine {
                    kind: 2,
                    pos: [7.0, 1.0, -3.0],
                    charge: 0.4,
                    state: 1,
                }],
                greed: 1,
                curse: 2,
                shop: vec![
                    Ware {
                        item: "storm_charm".into(),
                        price: 70,
                        sold: false,
                    },
                    Ware {
                        item: "lodestone".into(),
                        price: 35,
                        sold: true,
                    },
                ],
                stage: 2,
                zone: 7,
                gates: vec![Gate {
                    kind: 1,
                    pos: [3.0, 4.0, 5.0],
                    charge: 0.25,
                    open: true,
                }],
                swarm: 41.5,
            },
            Game::Me(Vitals {
                level: 9,
                xp: 12.0,
                hp: 88.0,
                abilities: vec![AbilityView {
                    key: "frostbolt".into(),
                    level: 300,
                    cd: 0.4,
                    damage: 2.5,
                    cooldown: 0.6,
                    targets: 3,
                    area: 0.4,
                    duration: 0.2,
                }],
                passives: vec![("might".into(), 2, 2.25)],
                pending_levels: 1,
                dead_t: Some(1.0),
                damage_done: 12345.0,
                kills: 50,
                picks: 4,
                loot: vec!["Hogger's Helm".into()],
                gold: 42.5,
                run_items: vec![("lodestone".into(), 3)],
                offer: vec![
                    Card {
                        choice: Choice::Upgrade("frostbolt".into()),
                        rarity: 4,
                        perks: vec![(0, 0.25), (2, 1.0)],
                    },
                    Card {
                        choice: Choice::Heal,
                        rarity: 0,
                        perks: vec![(0, 0.25), (2, 1.0)],
                    },
                ],
                rerolls: 2,
                banishes: 1,
                shrine_offer: vec![Boon {
                    bonus: (0, 0, 800),
                    rarity: 3,
                }],
                boons: vec![(11, 1, 2500), (7, 0, -1200)],
                keys: 2,
                damage_by: vec![(133, 1500.0), (0, 12.5)],
                ..Vitals::default()
            }),
            Game::Party(vec![PartyEntry {
                slot: 1,
                guid: 5,
                level: 3,
                hp: 50.0,
                max_hp: 120.0,
                dead: false,
            }]),
            Game::Outcome {
                won: true,
                scores: vec![Score {
                    slot: 0,
                    level: 20,
                    kills: 900,
                    damage_done: 1e6,
                }],
            },
            Game::Loaded,
            Game::Resync,
            Game::Input {
                t: 12.5,
                motion: motion(),
                haste: 1.6,
                haste_t: 2.0,
                vanish_t: 0.0,
            },
            Game::Card(CardAction::Pick(Card {
                choice: Choice::Upgrade("frostbolt".into()),
                rarity: 2,
                perks: vec![(0, 0.25), (2, 1.0)],
            })),
            Game::Card(CardAction::Reroll),
            Game::Card(CardAction::Banish(Card {
                choice: Choice::NewPassive("might".into()),
                rarity: 1,
                perks: vec![(0, 0.25), (2, 1.0)],
            })),
            Game::Card(CardAction::Skip),
            Game::ShrineChoice((4, 0, 500)),
            Game::Buy {
                slot: 2,
                item: "vampiric_fang".into(),
            },
            Game::Mobility {
                spell: 100,
                effect: MobilityEffect::Stun {
                    target: 3,
                    secs: 1.5,
                },
            },
            Game::Mobility {
                spell: 5484,
                effect: MobilityEffect::Fear {
                    at: [1.0, 0.0, 1.0],
                    radius: 12.0,
                    secs: 4.0,
                },
            },
        ]
    }

    #[test]
    fn every_game_message_round_trips() {
        for m in samples() {
            assert_eq!(Game::decode(&m.encode()).unwrap(), m, "{m:?}");
        }
    }

    #[test]
    fn batches_keep_order_and_skip_a_bad_message() {
        let a = Game::Loaded;
        let b = Game::Impact {
            target: 4,
            spell: 133,
        };
        let mut buf = encode_batch(&[a.clone(), b.clone()]);
        assert_eq!(decode_batch(&buf), vec![a.clone(), b.clone()]);
        // A garbage message in the middle is skipped, the rest still lands.
        let mut w = Writer::new();
        w.bytes(&[250, 1, 2]);
        let mut garbage = w.finish();
        garbage.extend(encode_batch(std::slice::from_ref(&b)));
        buf.extend(garbage);
        assert_eq!(decode_batch(&buf), vec![a, b.clone(), b]);
        assert!(decode_batch(&[]).is_empty());
        // A truncated tail ends the batch without a panic.
        assert!(decode_batch(&[9, 0, 0, 0, 1]).is_empty());
    }

    #[test]
    fn a_full_horde_fits_one_frame() {
        let list = vec![motion(); 400];
        let bytes = Game::Motions { t: 0.0, list }.encode();
        assert!(
            bytes.len() < crate::protocol::MAX_FRAME / 8,
            "{}",
            bytes.len()
        );
    }
}
