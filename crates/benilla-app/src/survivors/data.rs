//! The mode's tables: classes, abilities, passives, enemies and the wave clock. Spell ids name
//! `Spell.dbc` rows only for their icon, name and visual chain; every number here is the mode's.

/// A weapon on the hero's virtual item slots: `(slot, display, class, subclass, material, inv,
/// sheath)`, the creature form of `UNIT_VIRTUAL_ITEM_SLOT_DISPLAY` and `_INFO`.
pub(crate) type Weapon = (u16, u32, u8, u8, u8, u8, u8);

pub(crate) struct ClassDef {
    pub id: u8,
    pub name: &'static str,
    /// One line on how the class plays, for the class picker.
    pub blurb: &'static str,
    /// `RAID_CLASS_COLORS`, 0xRRGGBB.
    pub color: u32,
    /// `UNIT_FIELD_BYTES_0` byte 3.
    pub power: u8,
    pub start: &'static str,
    pub pool: &'static [&'static str],
    /// Class-trainer `CreatureDisplayInfo` ids, deduped by race and sex at the menu.
    pub looks: &'static [u32],
    pub weapons: &'static [Weapon],
    /// The sheath state the hero fights in: 1 melee, 2 ranged.
    pub sheath: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    /// A missile at the nearest enemies (or instant if the spell has no speed).
    Bolt,
    /// An instant hit on the nearest enemies in range.
    Strike,
    /// A close hit on the nearest enemies in melee reach.
    Melee,
    /// Everything in a radius around the hero.
    Nova,
    /// Everything in a cone toward the nearest enemy.
    Cone,
    /// A ground area at the densest nearby enemy, ticking for `duration`.
    Ground,
    /// A ground area under the hero, ticking for `duration`.
    GroundSelf,
    /// Jumps from enemy to enemy.
    Chain,
    /// A damage-over-time on several enemies.
    Dot,
    /// Always on: pulses every `cooldown` around the hero, with the spell's state visual on him.
    Aura,
    /// Always on: zaps one enemy close to the hero every `cooldown`.
    Orbit,
    /// Absorbs `damage` for `duration`.
    Shield,
    /// Immune for `duration`.
    Immune,
    /// Returns `damage` to every attacker for `duration`.
    Thorns,
    /// Heals `damage` over `duration`.
    Heal,
    /// Drops a totem that casts `sub` for `duration`.
    Totem,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Effect {
    None,
    /// Speed multiplier for seconds.
    Slow(f32, f32),
    Root(f32),
    Stun(f32),
    Fear(f32),
    /// Outgoing damage multiplier for seconds.
    Weaken(f32, f32),
    /// Heal the hero by this fraction of damage dealt.
    Drain(f32),
    /// Kill outright below this health fraction.
    Execute(f32),
}

pub(crate) struct AbilityDef {
    pub key: &'static str,
    pub spell: u32,
    pub kind: Kind,
    /// Direct damage; per tick for areas and pulses; absorb, heal or reflect for the self kinds.
    pub damage: f32,
    /// Damage over time, per second, for `duration`.
    pub dot: f32,
    pub cooldown: f32,
    pub range: f32,
    /// Splash for single hits, area for the area kinds, the jump for a chain.
    pub radius: f32,
    pub count: u32,
    pub duration: f32,
    pub effect: Effect,
    /// The spell whose state kit marks an affected enemy, 0 for none.
    pub mark: u32,
    /// A totem's display, and the spell it casts.
    pub totem: (u32, u32),
    /// The area's own visual hangs overhead and would hide the field from the camera, so the
    /// area plays the spell's impact on whatever it hits instead.
    pub overhead: bool,
}

const fn ab(key: &'static str, spell: u32, kind: Kind) -> AbilityDef {
    AbilityDef {
        key,
        spell,
        kind,
        damage: 0.0,
        dot: 0.0,
        cooldown: 2.0,
        range: 20.0,
        radius: 0.0,
        count: 1,
        duration: 0.0,
        effect: Effect::None,
        mark: 0,
        totem: (0, 0),
        overhead: false,
    }
}

macro_rules! ability {
    ($key:literal, $spell:literal, $kind:ident { $($f:ident: $v:expr),* $(,)? }) => {
        AbilityDef { $($f: $v,)* ..ab($key, $spell, Kind::$kind) }
    };
}

pub(crate) static ABILITIES: &[AbilityDef] = &[
    // --- Mage ---
    ability!(
        "frostbolt",
        116,
        Bolt {
            damage: 12.0,
            cooldown: 1.3,
            range: 24.0,
            effect: Effect::Slow(0.5, 2.5),
            mark: 116
        }
    ),
    ability!(
        "fireball",
        133,
        Bolt {
            damage: 20.0,
            cooldown: 2.4,
            range: 26.0,
            radius: 3.5
        }
    ),
    ability!(
        "arcane_missiles",
        7268,
        Bolt {
            damage: 6.0,
            cooldown: 1.9,
            range: 22.0,
            count: 3
        }
    ),
    ability!(
        "frost_nova",
        122,
        Nova {
            damage: 8.0,
            cooldown: 7.0,
            radius: 9.0,
            effect: Effect::Root(2.5),
            mark: 122
        }
    ),
    ability!(
        "arcane_explosion",
        1449,
        Nova {
            damage: 10.0,
            cooldown: 2.8,
            radius: 7.0
        }
    ),
    ability!(
        "blizzard",
        10,
        Ground {
            damage: 7.0,
            cooldown: 9.0,
            range: 20.0,
            radius: 7.0,
            duration: 6.0,
            effect: Effect::Slow(0.6, 1.5)
        }
    ),
    ability!(
        "flamestrike",
        2120,
        Ground {
            damage: 14.0,
            cooldown: 8.0,
            range: 20.0,
            radius: 5.0,
            duration: 4.0
        }
    ),
    ability!(
        "fire_blast",
        2136,
        Strike {
            damage: 26.0,
            cooldown: 3.5,
            range: 18.0
        }
    ),
    ability!(
        "cone_of_cold",
        120,
        Cone {
            damage: 14.0,
            cooldown: 4.5,
            radius: 11.0,
            effect: Effect::Slow(0.5, 3.0),
            mark: 120
        }
    ),
    ability!(
        "blast_wave",
        11113,
        Nova {
            damage: 18.0,
            cooldown: 6.5,
            radius: 8.0,
            effect: Effect::Slow(0.6, 2.0)
        }
    ),
    ability!(
        "pyroblast",
        11366,
        Bolt {
            damage: 70.0,
            dot: 6.0,
            duration: 6.0,
            cooldown: 6.0,
            range: 30.0
        }
    ),
    // --- Warrior ---
    ability!(
        "cleave",
        845,
        Melee {
            damage: 14.0,
            cooldown: 1.1,
            range: 5.5,
            count: 3
        }
    ),
    ability!(
        "revenge",
        6572,
        Melee {
            damage: 40.0,
            cooldown: 15.0,
            range: 5.5,
            duration: 10.0,
            kind: Kind::Thorns,
            count: 3
        }
    ),
    ability!(
        "heroic_strike",
        78,
        Melee {
            damage: 28.0,
            cooldown: 1.6,
            range: 5.5
        }
    ),
    ability!(
        "whirlwind",
        1680,
        Nova {
            damage: 16.0,
            cooldown: 4.0,
            radius: 7.0
        }
    ),
    ability!(
        "thunder_clap",
        6343,
        Nova {
            damage: 10.0,
            cooldown: 5.0,
            radius: 8.0,
            effect: Effect::Slow(0.5, 3.0),
            mark: 6343
        }
    ),
    ability!(
        "sweeping_strikes",
        12292,
        Aura {
            damage: 6.0,
            cooldown: 0.6,
            radius: 5.5
        }
    ),
    ability!(
        "demoralizing_shout",
        1160,
        Nova {
            damage: 4.0,
            cooldown: 8.0,
            radius: 11.0,
            effect: Effect::Weaken(0.5, 5.0)
        }
    ),
    ability!(
        "execute",
        5308,
        Strike {
            damage: 40.0,
            cooldown: 2.0,
            range: 8.0,
            count: 2,
            effect: Effect::Execute(0.3)
        }
    ),
    ability!(
        "mortal_strike",
        12294,
        Melee {
            damage: 48.0,
            cooldown: 3.0,
            range: 6.0
        }
    ),
    ability!(
        "retaliation",
        20230,
        Thorns {
            damage: 14.0,
            cooldown: 18.0,
            duration: 8.0
        }
    ),
    // --- Rogue ---
    ability!(
        "sinister_strike",
        1752,
        Melee {
            damage: 15.0,
            cooldown: 0.85,
            range: 5.5
        }
    ),
    ability!(
        "eviscerate",
        2098,
        Melee {
            damage: 42.0,
            cooldown: 2.8,
            range: 5.5
        }
    ),
    ability!(
        "blade_flurry",
        13877,
        Nova {
            damage: 11.0,
            cooldown: 3.0,
            radius: 6.0
        }
    ),
    ability!(
        "rupture",
        1943,
        Dot {
            dot: 5.0,
            cooldown: 3.5,
            range: 10.0,
            count: 3,
            duration: 6.0
        }
    ),
    ability!(
        "gouge",
        1776,
        Melee {
            damage: 8.0,
            cooldown: 4.0,
            range: 6.0,
            count: 2,
            effect: Effect::Stun(2.0),
            mark: 1776
        }
    ),
    ability!(
        "kidney_shot",
        408,
        Melee {
            damage: 12.0,
            cooldown: 6.0,
            range: 6.0,
            effect: Effect::Stun(3.5),
            mark: 408
        }
    ),
    // --- Priest ---
    ability!(
        "smite",
        585,
        Strike {
            damage: 14.0,
            cooldown: 1.2,
            range: 20.0
        }
    ),
    ability!(
        "shadow_word_pain",
        589,
        Dot {
            dot: 4.0,
            cooldown: 3.0,
            range: 18.0,
            count: 3,
            duration: 9.0
        }
    ),
    ability!(
        "holy_fire",
        14914,
        Strike {
            damage: 22.0,
            dot: 3.0,
            duration: 6.0,
            cooldown: 3.5,
            range: 20.0
        }
    ),
    ability!(
        "mind_blast",
        8092,
        Strike {
            damage: 32.0,
            cooldown: 3.5,
            range: 22.0
        }
    ),
    ability!(
        "holy_nova",
        15237,
        Nova {
            damage: 11.0,
            cooldown: 3.2,
            radius: 9.0,
            effect: Effect::Drain(0.1)
        }
    ),
    ability!(
        "psychic_scream",
        8122,
        Nova {
            damage: 2.0,
            cooldown: 12.0,
            radius: 8.0,
            effect: Effect::Fear(3.5),
            mark: 8122
        }
    ),
    ability!(
        "power_word_shield",
        17,
        Shield {
            damage: 30.0,
            cooldown: 14.0,
            duration: 10.0
        }
    ),
    // --- Warlock ---
    ability!(
        "shadow_bolt",
        686,
        Bolt {
            damage: 15.0,
            cooldown: 1.4,
            range: 24.0
        }
    ),
    ability!(
        "corruption",
        172,
        Dot {
            dot: 4.0,
            cooldown: 3.0,
            range: 20.0,
            count: 3,
            duration: 10.0,
            mark: 172
        }
    ),
    ability!(
        "curse_of_agony",
        980,
        Dot {
            dot: 3.5,
            cooldown: 2.5,
            range: 20.0,
            count: 4,
            duration: 12.0
        }
    ),
    ability!(
        "immolate",
        348,
        Strike {
            damage: 12.0,
            dot: 4.0,
            duration: 9.0,
            cooldown: 2.4,
            range: 20.0,
            count: 2,
            mark: 348
        }
    ),
    ability!(
        "rain_of_fire",
        5740,
        Ground {
            damage: 8.0,
            cooldown: 9.0,
            range: 20.0,
            radius: 7.0,
            duration: 6.0
        }
    ),
    ability!(
        "hellfire",
        5857,
        Aura {
            damage: 6.0,
            cooldown: 1.0,
            radius: 7.0
        }
    ),
    ability!(
        "death_coil",
        6789,
        Bolt {
            damage: 26.0,
            cooldown: 5.0,
            range: 22.0,
            effect: Effect::Drain(0.5)
        }
    ),
    ability!(
        "searing_pain",
        5676,
        Strike {
            damage: 18.0,
            cooldown: 1.8,
            range: 20.0
        }
    ),
    ability!(
        "shadowburn",
        17877,
        Strike {
            damage: 46.0,
            cooldown: 5.0,
            range: 20.0
        }
    ),
    // --- Hunter ---
    ability!(
        "arcane_shot",
        3044,
        Bolt {
            damage: 13.0,
            cooldown: 1.0,
            range: 28.0
        }
    ),
    ability!(
        "multi_shot",
        2643,
        Bolt {
            damage: 10.0,
            cooldown: 2.0,
            range: 26.0,
            count: 3
        }
    ),
    ability!(
        "serpent_sting",
        1978,
        Bolt {
            damage: 2.0,
            dot: 4.0,
            duration: 9.0,
            cooldown: 2.6,
            range: 26.0,
            count: 2
        }
    ),
    ability!(
        "volley",
        1510,
        Ground {
            damage: 8.0,
            cooldown: 9.0,
            range: 24.0,
            radius: 7.0,
            duration: 5.0
        }
    ),
    ability!(
        "explosive_trap",
        13812,
        Ground {
            damage: 22.0,
            dot: 4.0,
            cooldown: 8.0,
            range: 14.0,
            radius: 5.0,
            duration: 0.0
        }
    ),
    ability!(
        "concussive_shot",
        5116,
        Bolt {
            damage: 8.0,
            cooldown: 3.0,
            range: 28.0,
            effect: Effect::Slow(0.5, 4.0),
            mark: 5116
        }
    ),
    ability!(
        "aimed_shot",
        19434,
        Bolt {
            damage: 48.0,
            cooldown: 4.0,
            range: 32.0
        }
    ),
    // --- Shaman ---
    ability!(
        "lightning_bolt",
        403,
        Bolt {
            damage: 15.0,
            cooldown: 1.3,
            range: 24.0
        }
    ),
    ability!(
        "chain_lightning",
        421,
        Chain {
            damage: 18.0,
            cooldown: 3.0,
            range: 20.0,
            radius: 9.0,
            count: 3
        }
    ),
    ability!(
        "earth_shock",
        8042,
        Strike {
            damage: 22.0,
            cooldown: 2.5,
            range: 20.0
        }
    ),
    ability!(
        "flame_shock",
        8050,
        Strike {
            damage: 10.0,
            dot: 4.0,
            duration: 9.0,
            cooldown: 3.0,
            range: 20.0,
            count: 2
        }
    ),
    ability!(
        "frost_shock",
        8056,
        Strike {
            damage: 16.0,
            cooldown: 3.0,
            range: 20.0,
            effect: Effect::Slow(0.5, 4.0)
        }
    ),
    ability!(
        "searing_totem",
        3599,
        Totem {
            damage: 9.0,
            cooldown: 14.0,
            range: 20.0,
            duration: 14.0,
            totem: (4589, 3606)
        }
    ),
    ability!(
        "magma_totem",
        8190,
        Totem {
            damage: 8.0,
            cooldown: 16.0,
            radius: 7.0,
            duration: 14.0,
            totem: (4683, 8349)
        }
    ),
    ability!(
        "lightning_shield",
        324,
        Orbit {
            damage: 12.0,
            cooldown: 0.9,
            radius: 4.0
        }
    ),
    // --- Paladin ---
    ability!(
        "hammer_of_wrath",
        24275,
        Bolt {
            damage: 16.0,
            cooldown: 1.5,
            range: 24.0
        }
    ),
    ability!(
        "consecration",
        26573,
        GroundSelf {
            damage: 7.0,
            cooldown: 9.0,
            radius: 7.0,
            duration: 8.0
        }
    ),
    ability!(
        "holy_shock",
        25912,
        Strike {
            damage: 24.0,
            cooldown: 2.5,
            range: 20.0
        }
    ),
    ability!(
        "exorcism",
        879,
        Strike {
            damage: 36.0,
            cooldown: 5.0,
            range: 22.0
        }
    ),
    ability!(
        "holy_wrath",
        2812,
        Nova {
            damage: 14.0,
            cooldown: 5.0,
            radius: 11.0
        }
    ),
    ability!(
        "hammer_of_justice",
        853,
        Strike {
            damage: 8.0,
            cooldown: 6.0,
            range: 12.0,
            count: 2,
            effect: Effect::Stun(3.0),
            mark: 853
        }
    ),
    ability!(
        "divine_shield",
        642,
        Immune {
            cooldown: 28.0,
            duration: 4.0
        }
    ),
    ability!(
        "judgement",
        20271,
        Strike {
            damage: 26.0,
            cooldown: 4.0,
            range: 14.0,
            count: 2
        }
    ),
    // --- Druid ---
    ability!(
        "wrath",
        5176,
        Bolt {
            damage: 13.0,
            cooldown: 1.2,
            range: 24.0
        }
    ),
    ability!(
        "moonfire",
        8921,
        Strike {
            damage: 10.0,
            dot: 3.0,
            duration: 9.0,
            cooldown: 2.0,
            range: 22.0,
            count: 2
        }
    ),
    ability!(
        "starfire",
        2912,
        Strike {
            damage: 40.0,
            cooldown: 4.0,
            range: 24.0
        }
    ),
    ability!(
        "hurricane",
        16914,
        Ground {
            damage: 7.0,
            cooldown: 10.0,
            range: 20.0,
            radius: 8.0,
            duration: 7.0,
            effect: Effect::Slow(0.6, 1.5),
            overhead: true
        }
    ),
    ability!(
        "insect_swarm",
        5570,
        Dot {
            dot: 4.0,
            cooldown: 3.0,
            range: 20.0,
            count: 3,
            duration: 10.0,
            mark: 5570
        }
    ),
    ability!(
        "entangling_roots",
        339,
        Dot {
            dot: 2.0,
            cooldown: 6.0,
            range: 16.0,
            count: 4,
            duration: 5.0,
            effect: Effect::Root(4.0),
            mark: 339
        }
    ),
    ability!(
        "thorns",
        467,
        Thorns {
            damage: 8.0,
            cooldown: 15.0,
            duration: 15.0
        }
    ),
    ability!(
        "rejuvenation",
        774,
        Heal {
            damage: 30.0,
            cooldown: 15.0,
            duration: 10.0
        }
    ),
];

pub(crate) fn ability(key: &str) -> &'static AbilityDef {
    ABILITIES
        .iter()
        .find(|a| a.key == key)
        .unwrap_or_else(|| panic!("survivors: no ability {key:?}"))
}

pub(crate) const MAX_PASSIVE_LEVEL: u8 = 5;
pub(crate) const MAX_ABILITIES: usize = 6;
pub(crate) const MAX_PASSIVES: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Stat {
    Might,
    MaxHp,
    Speed,
    Regen,
    Haste,
    Area,
    Magnet,
    Growth,
    Armor,
    Crit,
    /// The fraction of every hit taken that is ignored, after armour; capped at
    /// [`MAX_DAMAGE_REDUCTION`].
    DamageReduction,
    /// The multiplier on gold picked up.
    Gold,
}

impl Stat {
    pub const ALL: [Stat; 12] = [
        Stat::Might,
        Stat::MaxHp,
        Stat::Speed,
        Stat::Regen,
        Stat::Haste,
        Stat::Area,
        Stat::Magnet,
        Stat::Growth,
        Stat::Armor,
        Stat::Crit,
        Stat::DamageReduction,
        Stat::Gold,
    ];

    /// A hero's number for this stat before any bonus.
    pub const fn base(self) -> f32 {
        match self {
            Stat::Might | Stat::Area | Stat::Growth | Stat::Gold => 1.0,
            Stat::MaxHp => 120.0,
            Stat::Speed => 7.0,
            Stat::Magnet => 4.5,
            Stat::Crit => 0.05,
            Stat::Regen | Stat::Haste | Stat::Armor | Stat::DamageReduction => 0.0,
        }
    }
}

/// The most of a hit damage reduction can ignore.
pub(crate) const MAX_DAMAGE_REDUCTION: f32 = 0.75;

/// How a bonus adds to a stat: every flat amount is summed onto the base, then the sum of every
/// percent multiplies it — `(base + Σflat) × (1 + Σpercent)`. A stat that is itself a fraction
/// (crit, haste, damage reduction) takes flat fractions: +1 % crit is `Flat(0.01)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Amount {
    Flat(f32),
    Percent(f32),
}

/// One bonus to one stat, from whatever source: a passive's rank, the class, the collection, a
/// talent, a racial, a piece of gear, a running effect.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Bonus {
    pub stat: Stat,
    pub amount: Amount,
}

impl Bonus {
    pub const fn flat(stat: Stat, v: f32) -> Self {
        Self {
            stat,
            amount: Amount::Flat(v),
        }
    }
    pub const fn pct(stat: Stat, v: f32) -> Self {
        Self {
            stat,
            amount: Amount::Percent(v),
        }
    }
}

/// An effect that fires on its own every `every` seconds and grants `bonuses` for `duration`
/// (a racial like Berserking, a talent, a set bonus); `spell` names its icon and its visual on
/// the hero.
pub(crate) struct ProcDef {
    pub key: &'static str,
    pub spell: u32,
    pub every: f32,
    pub duration: f32,
    pub bonuses: &'static [Bonus],
}

/// One of a race's three racial traits: bonuses that hold all run, an effect that fires on its
/// own, or both. `spell` names the vanilla racial's `Spell.dbc` row for icon and visual; the
/// numbers are the mode's.
pub(crate) struct RacialDef {
    pub key: &'static str,
    pub name: &'static str,
    pub spell: u32,
    /// What it does, as the main screen's tooltip reads it.
    pub text: &'static str,
    pub bonuses: &'static [Bonus],
    pub proc: Option<&'static ProcDef>,
}

const fn racial(
    key: &'static str,
    name: &'static str,
    spell: u32,
    text: &'static str,
    bonuses: &'static [Bonus],
) -> RacialDef {
    RacialDef {
        key,
        name,
        spell,
        text,
        bonuses,
        proc: None,
    }
}

const fn racial_proc(name: &'static str, text: &'static str, proc: &'static ProcDef) -> RacialDef {
    RacialDef {
        key: proc.key,
        name,
        spell: proc.spell,
        text,
        bonuses: &[],
        proc: Some(proc),
    }
}

static BLOOD_FURY: ProcDef = ProcDef {
    key: "blood_fury",
    spell: 20572,
    every: 40.0,
    duration: 15.0,
    bonuses: &[Bonus::flat(Stat::Might, 0.25)],
};
static STONEFORM: ProcDef = ProcDef {
    key: "stoneform",
    spell: 20594,
    every: 45.0,
    duration: 8.0,
    bonuses: &[Bonus::flat(Stat::DamageReduction, 0.25)],
};
static SHADOWMELD: ProcDef = ProcDef {
    key: "shadowmeld",
    spell: 20580,
    every: 40.0,
    duration: 5.0,
    bonuses: &[Bonus::flat(Stat::DamageReduction, 0.4)],
};
static WILL_OF_THE_FORSAKEN: ProcDef = ProcDef {
    key: "will_of_the_forsaken",
    spell: 7744,
    every: 45.0,
    duration: 5.0,
    bonuses: &[Bonus::flat(Stat::DamageReduction, 0.5)],
};
static WAR_STOMP: ProcDef = ProcDef {
    key: "war_stomp",
    spell: 20549,
    every: 30.0,
    duration: 6.0,
    bonuses: &[Bonus::flat(Stat::Armor, 6.0)],
};
static BERSERKING: ProcDef = ProcDef {
    key: "berserking",
    spell: 26297,
    every: 35.0,
    duration: 10.0,
    bonuses: &[Bonus::flat(Stat::Haste, 0.3)],
};

/// The three racials of a race (`ChrRaces.dbc` id), after vanilla's own.
pub(crate) fn racials(race: u8) -> &'static [RacialDef; 3] {
    static HUMAN: [RacialDef; 3] = [
        racial(
            "diplomacy",
            "Diplomacy",
            20599,
            "+5% gold",
            &[Bonus::pct(Stat::Gold, 0.05)],
        ),
        racial(
            "weapon_specialization",
            "Sword and Mace Specialization",
            20597,
            "+4% damage",
            &[Bonus::flat(Stat::Might, 0.04)],
        ),
        racial(
            "the_human_spirit",
            "The Human Spirit",
            20598,
            "+5% experience",
            &[Bonus::flat(Stat::Growth, 0.05)],
        ),
    ];
    static ORC: [RacialDef; 3] = [
        racial_proc(
            "Blood Fury",
            "Every 40 sec, +25% damage for 15 sec",
            &BLOOD_FURY,
        ),
        racial(
            "hardiness",
            "Hardiness",
            20573,
            "+2% damage reduction",
            &[Bonus::flat(Stat::DamageReduction, 0.02)],
        ),
        racial(
            "axe_specialization",
            "Axe Specialization",
            20574,
            "+2% critical strike",
            &[Bonus::flat(Stat::Crit, 0.02)],
        ),
    ];
    static DWARF: [RacialDef; 3] = [
        racial_proc(
            "Stoneform",
            "Every 45 sec, 25% less damage taken for 8 sec",
            &STONEFORM,
        ),
        racial(
            "treasure_finding",
            "Treasure Finding",
            2481,
            "+5% gold",
            &[Bonus::pct(Stat::Gold, 0.05)],
        ),
        racial(
            "gun_specialization",
            "Gun Specialization",
            20595,
            "+2% critical strike",
            &[Bonus::flat(Stat::Crit, 0.02)],
        ),
    ];
    static NIGHT_ELF: [RacialDef; 3] = [
        racial_proc(
            "Shadowmeld",
            "Every 40 sec, 40% less damage taken for 5 sec",
            &SHADOWMELD,
        ),
        racial(
            "quickness",
            "Quickness",
            20582,
            "+2% damage reduction",
            &[Bonus::flat(Stat::DamageReduction, 0.02)],
        ),
        racial(
            "wisp_spirit",
            "Wisp Spirit",
            20585,
            "+5% movement speed",
            &[Bonus::pct(Stat::Speed, 0.05)],
        ),
    ];
    static UNDEAD: [RacialDef; 3] = [
        racial_proc(
            "Will of the Forsaken",
            "Every 45 sec, 50% less damage taken for 5 sec",
            &WILL_OF_THE_FORSAKEN,
        ),
        racial(
            "cannibalize",
            "Cannibalize",
            20577,
            "+1 health per second",
            &[Bonus::flat(Stat::Regen, 1.0)],
        ),
        racial(
            "shadow_resistance",
            "Shadow Resistance",
            20579,
            "+2% damage reduction",
            &[Bonus::flat(Stat::DamageReduction, 0.02)],
        ),
    ];
    static TAUREN: [RacialDef; 3] = [
        racial(
            "endurance",
            "Endurance",
            20550,
            "+5% max health",
            &[Bonus::pct(Stat::MaxHp, 0.05)],
        ),
        racial_proc(
            "War Stomp",
            "Every 30 sec, +6 damage blocked per hit for 6 sec",
            &WAR_STOMP,
        ),
        racial(
            "cultivation",
            "Cultivation",
            20552,
            "+8% pickup range",
            &[Bonus::pct(Stat::Magnet, 0.08)],
        ),
    ];
    static GNOME: [RacialDef; 3] = [
        racial(
            "expansive_mind",
            "Expansive Mind",
            20591,
            "+5% experience",
            &[Bonus::flat(Stat::Growth, 0.05)],
        ),
        racial(
            "escape_artist",
            "Escape Artist",
            20589,
            "+5% movement speed",
            &[Bonus::pct(Stat::Speed, 0.05)],
        ),
        racial(
            "engineering_specialization",
            "Engineering Specialization",
            20593,
            "+3% area of effect",
            &[Bonus::flat(Stat::Area, 0.03)],
        ),
    ];
    static TROLL: [RacialDef; 3] = [
        racial_proc(
            "Berserking",
            "Every 35 sec, cooldowns recover 30% faster for 10 sec",
            &BERSERKING,
        ),
        racial(
            "regeneration",
            "Regeneration",
            20555,
            "+1.5 health per second",
            &[Bonus::flat(Stat::Regen, 1.5)],
        ),
        racial(
            "throwing_specialization",
            "Throwing Specialization",
            20558,
            "+3% critical strike",
            &[Bonus::flat(Stat::Crit, 0.03)],
        ),
    ];
    match race {
        2 => &ORC,
        3 => &DWARF,
        4 => &NIGHT_ELF,
        5 => &UNDEAD,
        6 => &TAUREN,
        7 => &GNOME,
        8 => &TROLL,
        _ => &HUMAN,
    }
}

/// What a race's racials bring into a run: the bonuses that hold all run, and the effects that
/// fire on their own.
pub(crate) fn racial_bonuses(race: u8) -> (Vec<Bonus>, Vec<&'static ProcDef>) {
    let mut bonuses = Vec::new();
    let mut procs = Vec::new();
    for r in racials(race) {
        bonuses.extend_from_slice(r.bonuses);
        procs.extend(r.proc);
    }
    (bonuses, procs)
}

/// The classes (`ChrClasses.dbc` ids) a race could be in vanilla, for when the client's own
/// table of looks is not loaded.
pub(crate) fn race_classes(race: u8) -> &'static [u8] {
    match race {
        1 => &[1, 2, 4, 5, 8, 9],
        2 => &[1, 3, 4, 7, 9],
        3 => &[1, 2, 3, 4, 5],
        4 => &[1, 3, 4, 5, 11],
        5 => &[1, 4, 5, 8, 9],
        6 => &[1, 3, 7, 11],
        7 => &[1, 4, 8, 9],
        8 => &[1, 3, 4, 5, 7, 8],
        _ => &[],
    }
}

/// The class's own head start: tanky classes tougher, cloth quicker.
pub(crate) fn class_bonuses(class_id: u8) -> &'static [Bonus] {
    const TANKY: &[Bonus] = &[Bonus::flat(Stat::MaxHp, 40.0)];
    const MAIL: &[Bonus] = &[Bonus::flat(Stat::MaxHp, 15.0)];
    const CLOTH: &[Bonus] = &[Bonus::flat(Stat::Speed, 0.5)];
    match class_id {
        1 | 2 => TANKY,
        7 | 11 | 3 | 4 => MAIL,
        _ => CLOTH,
    }
}

pub(crate) struct PassiveDef {
    pub key: &'static str,
    pub name: &'static str,
    /// Icon source, and a state visual on the hero where the spell has one.
    pub spell: u32,
    pub stat: Stat,
    pub per_level: f32,
}

impl PassiveDef {
    /// The passive at `level`: health, speed and pickup range grow by a percent of themselves,
    /// every other stat by its own amount.
    pub fn bonus(&self, level: u8) -> Bonus {
        self.bonus_at(f32::from(level))
    }

    /// The passive at `power` ranks' worth: its ranks, each weighed by its card's rarity.
    pub fn bonus_at(&self, power: f32) -> Bonus {
        let v = self.per_level * power;
        match self.stat {
            Stat::MaxHp | Stat::Speed | Stat::Magnet => Bonus::pct(self.stat, v),
            _ => Bonus::flat(self.stat, v),
        }
    }
}

pub(crate) static PASSIVES: &[PassiveDef] = &[
    PassiveDef {
        key: "might",
        name: "Blessing of Might",
        spell: 19740,
        stat: Stat::Might,
        per_level: 0.10,
    },
    PassiveDef {
        key: "fortitude",
        name: "Power Word: Fortitude",
        spell: 1243,
        stat: Stat::MaxHp,
        per_level: 0.20,
    },
    PassiveDef {
        key: "pack",
        name: "Aspect of the Pack",
        spell: 13159,
        stat: Stat::Speed,
        per_level: 0.10,
    },
    PassiveDef {
        key: "renew",
        name: "Renew",
        spell: 139,
        stat: Stat::Regen,
        per_level: 0.5,
    },
    PassiveDef {
        key: "intellect",
        name: "Arcane Intellect",
        spell: 1459,
        stat: Stat::Haste,
        per_level: 0.08,
    },
    PassiveDef {
        key: "amplify",
        name: "Amplify Magic",
        spell: 1008,
        stat: Stat::Area,
        per_level: 0.10,
    },
    PassiveDef {
        key: "detect",
        name: "Detect Lesser Invisibility",
        spell: 132,
        stat: Stat::Magnet,
        per_level: 0.40,
    },
    PassiveDef {
        key: "wisdom",
        name: "Blessing of Wisdom",
        spell: 19742,
        stat: Stat::Growth,
        per_level: 0.08,
    },
    PassiveDef {
        key: "wild",
        name: "Mark of the Wild",
        spell: 1126,
        stat: Stat::Armor,
        per_level: 1.0,
    },
    PassiveDef {
        key: "kings",
        name: "Blessing of Kings",
        spell: 20217,
        stat: Stat::Crit,
        per_level: 0.05,
    },
];

pub(crate) static CLASSES: &[ClassDef] = &[
    ClassDef {
        id: 1,
        name: "Warrior",
        blurb: "Steel and fury: cleave through the horde and shrug off its blows.",
        color: 0xC79C6E,
        power: 1,
        start: "cleave",
        pool: &[
            "cleave",
            "heroic_strike",
            "whirlwind",
            "thunder_clap",
            "sweeping_strikes",
            "demoralizing_shout",
            "execute",
            "mortal_strike",
            "retaliation",
            "revenge",
        ],
        looks: &[
            1300, 1374, 1375, 1504, 1578, 1599, 1707, 1721, 1880, 2096, 2103, 2113, 2196, 2198,
            2614, 2620, 2658, 3053, 3054, 3055, 3280, 3287, 3343, 3399, 3431, 3743, 3793, 3794,
            4242, 4556, 6071, 11037,
        ],
        weapons: &[(0, 7483, 2, 7, 1, 13, 3), (1, 1706, 4, 6, 1, 14, 4)],
        sheath: 1,
    },
    ClassDef {
        id: 2,
        name: "Paladin",
        blurb: "Holy light that smites the wicked and shields the faithful.",
        color: 0xF58CBA,
        power: 0,
        start: "hammer_of_wrath",
        pool: &[
            "hammer_of_wrath",
            "consecration",
            "holy_shock",
            "exorcism",
            "holy_wrath",
            "hammer_of_justice",
            "divine_shield",
            "judgement",
        ],
        looks: &[
            1299, 1499, 1622, 3087, 3088, 3089, 3284, 3289, 3346, 3393, 7356,
        ],
        weapons: &[(0, 8011, 2, 4, 2, 13, 3), (1, 1705, 4, 6, 1, 14, 4)],
        sheath: 1,
    },
    ClassDef {
        id: 3,
        name: "Hunter",
        blurb: "Shots that strike from afar while the horde gives chase.",
        color: 0xABD473,
        power: 0,
        start: "arcane_shot",
        pool: &[
            "arcane_shot",
            "multi_shot",
            "serpent_sting",
            "volley",
            "explosive_trap",
            "concussive_shot",
            "aimed_shot",
        ],
        looks: &[
            1373, 1703, 1723, 1882, 2066, 2087, 2105, 2112, 2205, 2206, 2251, 3056, 3072, 3073,
            3299, 3309, 3310, 3312, 3395, 3558, 3744, 3810, 3811, 4239, 4241, 4372, 4560, 7538,
            10245,
        ],
        weapons: &[(0, 7480, 2, 6, 1, 17, 2), (2, 6235, 2, 2, 2, 15, 0)],
        sheath: 2,
    },
    ClassDef {
        id: 4,
        name: "Rogue",
        blurb: "Blades and speed: strike first, strike hard, and be gone.",
        color: 0xFFF569,
        power: 3,
        start: "sinister_strike",
        pool: &[
            "sinister_strike",
            "eviscerate",
            "blade_flurry",
            "rupture",
            "gouge",
            "kidney_shot",
        ],
        looks: &[
            1297, 1327, 1328, 1507, 1580, 1603, 1704, 1725, 1886, 2231, 2243, 2252, 2631, 2639,
            2659, 3100, 3101, 3113, 3351, 3407, 3436, 3749, 4360, 5146, 5528, 13171,
        ],
        weapons: &[(0, 7492, 2, 7, 1, 13, 3), (1, 6443, 2, 15, 1, 13, 3)],
        sheath: 1,
    },
    ClassDef {
        id: 5,
        name: "Priest",
        blurb: "Light that burns the horde and mends the one who wields it.",
        color: 0xFFFFFF,
        power: 0,
        start: "smite",
        pool: &[
            "smite",
            "shadow_word_pain",
            "holy_fire",
            "mind_blast",
            "holy_nova",
            "psychic_scream",
            "power_word_shield",
        ],
        looks: &[
            1295, 1495, 1579, 1602, 1708, 1733, 1897, 2137, 2138, 2139, 2200, 2201, 2202, 2618,
            2626, 3066, 3085, 3086, 3282, 3283, 3344, 3401, 3429, 4068, 4690, 4711, 10473, 10723,
            11044, 11048, 11053,
        ],
        weapons: &[(0, 1926, 2, 10, 2, 17, 2)],
        sheath: 1,
    },
    ClassDef {
        id: 7,
        name: "Shaman",
        blurb: "Storms that jump from foe to foe, and totems that fight beside you.",
        color: 0x0070DE,
        power: 0,
        start: "lightning_bolt",
        pool: &[
            "lightning_bolt",
            "chain_lightning",
            "earth_shock",
            "flame_shock",
            "frost_shock",
            "searing_totem",
            "magma_totem",
            "lightning_shield",
        ],
        looks: &[
            1360, 1878, 2082, 2102, 2123, 3746, 3816, 4231, 4552, 10180, 13341,
        ],
        weapons: &[(0, 7477, 2, 4, 2, 13, 3), (1, 1705, 4, 6, 1, 14, 4)],
        sheath: 1,
    },
    ClassDef {
        id: 8,
        name: "Mage",
        blurb: "Frost, fire and arcane: freeze the horde, then burn it down.",
        color: 0x69CCF0,
        power: 0,
        start: "frostbolt",
        pool: &[
            "frostbolt",
            "fireball",
            "arcane_missiles",
            "frost_nova",
            "arcane_explosion",
            "blizzard",
            "flamestrike",
            "fire_blast",
            "cone_of_cold",
            "blast_wave",
            "pyroblast",
        ],
        looks: &[
            1294, 1470, 1484, 1592, 1600, 2134, 2135, 2644, 2657, 2810, 3108, 3109, 3292, 3293,
            4522, 4523, 4524, 4526, 4665, 5001, 6058, 6060, 6072, 7669, 10171, 10214, 10215, 10216,
            10548, 10733, 12849,
        ],
        weapons: &[(0, 1927, 2, 10, 2, 17, 2)],
        sheath: 1,
    },
    ClassDef {
        id: 9,
        name: "Warlock",
        blurb: "Curses that eat at the horde, and terror that scatters it.",
        color: 0x9482C9,
        power: 0,
        start: "shadow_bolt",
        pool: &[
            "shadow_bolt",
            "corruption",
            "curse_of_agony",
            "immolate",
            "rain_of_fire",
            "hellfire",
            "death_coil",
            "searing_pain",
            "shadowburn",
        ],
        looks: &[
            1324, 1325, 1326, 1469, 1581, 1604, 1884, 1930, 2637, 2646, 2675, 3115, 3116, 3122,
            3271, 3286, 3291, 3345, 3607, 3745, 4567,
        ],
        weapons: &[(0, 5542, 2, 10, 2, 17, 2)],
        sheath: 1,
    },
    ClassDef {
        id: 11,
        name: "Druid",
        blurb: "Nature's wrath, and the wild's own strength to weather it.",
        color: 0xFF7D0A,
        power: 0,
        start: "wrath",
        pool: &[
            "wrath",
            "moonfire",
            "starfire",
            "hurricane",
            "insect_swarm",
            "entangling_roots",
            "thorns",
            "rejuvenation",
        ],
        looks: &[
            1706, 1732, 2106, 2115, 2121, 2250, 2255, 2261, 3300, 3301, 3302, 3819, 3820, 7357,
            10738, 12053,
        ],
        weapons: &[(0, 22391, 2, 10, 2, 17, 2)],
        sheath: 1,
    },
];

#[derive(Clone, Copy)]
pub(crate) struct EnemyDef {
    pub name: &'static str,
    pub display: u32,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub scale: f32,
    pub xp: u32,
    pub level: u32,
    /// A great beast, which takes more room in the crowd.
    pub big: bool,
}

const fn en(
    name: &'static str,
    display: u32,
    hp: f32,
    speed: f32,
    damage: f32,
    xp: u32,
    level: u32,
) -> EnemyDef {
    EnemyDef {
        name,
        display,
        hp,
        speed,
        damage,
        scale: 1.0,
        xp,
        level,
        big: false,
    }
}

/// The Barrens, roughly in the order a character meets them.
pub(crate) static ENEMIES: &[EnemyDef] = &[
    en("Fleeting Plainstrider", 1284, 8.0, 3.6, 2.0, 1, 10),
    en("Zhevra Runner", 6087, 10.0, 4.4, 2.0, 1, 11),
    en("Razormane Quilboar", 1218, 16.0, 3.4, 3.0, 2, 12),
    en("Razormane Hunter", 6094, 18.0, 3.6, 4.0, 2, 12),
    en("Savannah Huntress", 1056, 22.0, 4.8, 8.0, 2, 13),
    en("Sunscale Lashtail", 1744, 26.0, 4.6, 8.0, 3, 13),
    en("Kolkar Wrangler", 9442, 36.0, 4.0, 10.0, 3, 14),
    en("Kolkar Stormer", 9443, 40.0, 4.0, 11.0, 3, 14),
    en("Witchwing Harpy", 3218, 34.0, 5.0, 10.0, 3, 15),
    en("Hecklefang Hyena", 2710, 44.0, 5.2, 12.0, 4, 15),
    en("Razormane Defender", 1253, 70.0, 3.3, 14.0, 5, 16),
    en("Razormane Mystic", 4643, 55.0, 3.8, 12.0, 4, 16),
    en("Sunscale Screecher", 1747, 60.0, 5.0, 13.0, 4, 17),
    en("Kolkar Bloodcharger", 9447, 85.0, 4.6, 16.0, 5, 18),
    en("Savannah Prowler", 1973, 75.0, 5.4, 15.0, 5, 18),
    en("Thunderhawk Hatchling", 1742, 90.0, 4.2, 16.0, 6, 19),
    en("Witchwing Slayer", 2163, 95.0, 5.0, 18.0, 6, 20),
    en("Bristleback Geomancer", 6091, 120.0, 3.6, 20.0, 7, 21),
    huge(en("Stormsnout", 1537, 150.0, 4.0, 22.0, 8, 22)),
    huge(en("Barrens Kodo", 1453, 220.0, 3.2, 28.0, 12, 23)),
    en("Sunscale Scytheclaw", 4442, 160.0, 5.6, 24.0, 8, 24),
    huge(en("Thunderhead", 1538, 180.0, 4.2, 26.0, 10, 25)),
    en("Silithid Swarmer", 2731, 90.0, 6.0, 18.0, 5, 25),
    huge(en("Greater Thunderhawk", 1974, 240.0, 4.4, 30.0, 12, 26)),
    en("Hecklefang Stalker", 2712, 200.0, 6.0, 30.0, 10, 27),
    en("Kolkar Destroyer", 9446, 280.0, 4.6, 34.0, 14, 28),
    en("Kolkar Mauler", 4874, 300.0, 4.6, 36.0, 14, 29),
    huge(en("Greater Barrens Kodo", 1232, 420.0, 3.4, 42.0, 24, 30)),
    huge(en("Elder Thunder Lizard", 2764, 380.0, 4.4, 44.0, 22, 31)),
    huge(en("Silithid Protector", 1307, 360.0, 5.0, 40.0, 20, 32)),
];

/// The great beasts at three quarters size, so a crowd of them still leaves the field readable.
const fn huge(def: EnemyDef) -> EnemyDef {
    EnemyDef {
        scale: 0.75,
        big: true,
        ..def
    }
}

pub(crate) static BOSSES: &[EnemyDef] = &[
    EnemyDef {
        scale: 1.6,
        ..en("Echeyakee", 1934, 700.0, 5.2, 20.0, 60, 16)
    },
    EnemyDef {
        scale: 1.6,
        ..en("Takk the Leaper", 1337, 1800.0, 5.4, 32.0, 120, 19)
    },
    EnemyDef {
        scale: 1.7,
        ..en("Lakota'mani", 1241, 4200.0, 4.0, 45.0, 200, 22)
    },
    EnemyDef {
        scale: 1.7,
        ..en("Humar the Pridelord", 4424, 7000.0, 5.6, 55.0, 300, 26)
    },
    EnemyDef {
        scale: 1.8,
        ..en("Agathelos the Raging", 2450, 12000.0, 5.0, 70.0, 400, 33)
    },
    EnemyDef {
        scale: 1.8,
        ..en("Hezrul Bloodmark", 9448, 20000.0, 4.8, 90.0, 500, 33)
    },
];

/// When the final boss comes; the run is won when it falls.
pub(crate) const FINAL_BOSS_SECS: f32 = 960.0;
/// When each of a zone's five bosses comes under the solo rules, the last the final boss. A
/// lobby's own length stretches or shrinks them all alike.
pub(crate) const BOSS_TIMES: [f32; 5] = [180.0, 360.0, 540.0, 720.0, FINAL_BOSS_SECS];

/// A run is this many stages, each a zone of the campaign from the one picked on, then the final
/// swarm: hold out against it [`SWARM_SECS`] to win.
pub(crate) const STAGES: u8 = 3;
/// A stage's clock against the zone's own: its bosses come at their slots of this much of the
/// run's length, and the boss portal opens at the final boss's.
pub(crate) const STAGE_LENGTH: f32 = 0.6;
/// What each stage after the first does to the foes, again per stage.
pub(crate) const STAGE_HEALTH: f32 = 1.6;
pub(crate) const STAGE_DAMAGE: f32 = 1.35;
pub(crate) const STAGE_DENSITY: f32 = 1.15;
/// Seconds a hero stands in a portal to wake it, and how near.
pub(crate) const PORTAL_HOLD: f32 = 3.0;
pub(crate) const PORTAL_RADIUS: f32 = 4.0;
/// What a stage's end gives every hero: gold, scaled by the stage, a key, and a blessing to choose.
pub(crate) const STAGE_GOLD: u32 = 50;
/// The final swarm: how long to hold, and how much thicker the crowd.
pub(crate) const SWARM_SECS: f32 = 60.0;
pub(crate) const SWARM_DENSITY: f32 = 3.0;
/// The portals' marks: the boss portal's and the stage portal's colours.
pub(crate) const BOSS_PORTAL_COLOR: [f32; 3] = [0.9, 0.15, 0.2];
pub(crate) const STAGE_PORTAL_COLOR: [f32; 3] = [0.3, 0.6, 1.0];
/// Their icons, `Spell.dbc` rows for the icon only: Ritual of Summoning, and Teleport: Stormwind.
pub(crate) const BOSS_PORTAL_ICON: u32 = 698;
pub(crate) const STAGE_PORTAL_ICON: u32 = 3561;

/// The zone stage `stage` (from 0) of a run begun in `first` stands in: the campaign's next ones,
/// the last zone again once past the end.
pub(crate) fn stage_zone(first: usize, stage: u8) -> usize {
    (first + usize::from(stage)).min(MAPS.len() - 1)
}
/// A closing ring of the horde every two and a quarter minutes, half way into each interval.
pub(crate) const AMBUSH_EVERY_SECS: f32 = 135.0;
/// A gem caught by a magnet flies off at this speed (yd/s) and gains this much a second.
pub(crate) const GEM_SPEED: f32 = 10.0;
pub(crate) const GEM_ACCEL: f32 = 60.0;
/// Seconds a stage's fall pulls every gem on the field to the nearest hero.
pub(crate) const VACUUM_SECS: f32 = 2.5;

/// Experience per gem: every level of the curve comes at the run's pace.
pub(crate) const XP_RATE: f32 = 2.0;

/// The most enemies alive at once, whatever the density: past it the frame budget, not the
/// challenge, is what grows.
pub(crate) const HARD_CROWD_CAP: usize = 400;

/// What the horde's blows grow by over a stage's clock (0.9 before the simulator: the late
/// stage killed a fresh hero in a second of contact).
pub(crate) const HORDE_DAMAGE_GROWTH: f32 = 0.6;

/// The horde comes in waves: every [`WAVE_EVERY`] seconds of a stage, the last [`SURGE_SECS`]
/// of it are a surge, the crowd [`SURGE_CROWD`] times as thick and coming in packs; between
/// surges it thins to [`LULL_CROWD`].
pub(crate) const WAVE_EVERY: f32 = 45.0;
pub(crate) const SURGE_SECS: f32 = 12.0;
pub(crate) const SURGE_CROWD: f32 = 1.35;
pub(crate) const LULL_CROWD: f32 = 0.9;
/// An ambush ring leaves this many openings, each this wide (radians), turned at random: a
/// closed ring is a death no player can see coming or get out of.
pub(crate) const RING_GAPS: usize = 2;
pub(crate) const RING_GAP: f32 = 0.7;

/// The angles of an ambush ring of `n` foes turned by `turn`: [`RING_GAPS`] arcs, evenly
/// filled, with a [`RING_GAP`] opening after each.
pub(crate) fn ring_angles(n: usize, turn: f32) -> Vec<f32> {
    let arc = std::f32::consts::TAU / RING_GAPS as f32;
    let filled = arc - RING_GAP;
    let per = n.div_ceil(RING_GAPS).max(1);
    (0..n)
        .map(|i| {
            let (seg, k) = (i / per, i % per);
            turn + seg as f32 * arc + filled * (k as f32 + 0.5) / per as f32
        })
        .collect()
}

/// A surge comes from one side of the heroes, its packs within this many radians of it, the side
/// turning this many radians a second as the surge goes on.
pub(crate) const SURGE_ARC: f32 = 0.6;
pub(crate) const SURGE_DRIFT: f32 = 0.12;

/// A surge's spawns come this many together, from one side of a hero.
pub(crate) const PACK_SIZE: usize = 8;
/// The crowd a stage keeps: from [`CROWD_START`] to [`CROWD_PEAK`] over its clock, before the
/// waves and the density.
pub(crate) const CROWD_START: f32 = 24.0;
pub(crate) const CROWD_PEAK: f32 = 270.0;

/// A run's tunable rules: the solo defaults, or a lobby's settings for its number of heroes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rules {
    /// When the final boss comes ([`FINAL_BOSS_SECS`] solo); every boss's time scales with it.
    pub duration: f32,
    /// How often the horde closes a ring around every hero.
    pub ambush_every: f32,
    /// Multipliers on enemy health, enemy damage and the crowd's size.
    pub health: f32,
    pub damage: f32,
    pub density: f32,
    /// Health and crowd grow with the heroes in the run.
    pub scale_with_heroes: bool,
    pub heroes: u32,
    /// Experience any hero picks up goes to every hero.
    pub shared_xp: bool,
    /// An ally standing over a fallen hero brings them back.
    pub revive: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self::SOLO
    }
}

impl Rules {
    pub const SOLO: Self = Self {
        duration: FINAL_BOSS_SECS,
        ambush_every: AMBUSH_EVERY_SECS,
        health: 1.0,
        damage: 1.0,
        density: 1.0,
        scale_with_heroes: true,
        heroes: 1,
        shared_xp: true,
        revive: false,
    };

    /// How far through the run, 0 at the start and 1 when the final boss comes.
    pub fn progress(&self, elapsed: f32) -> f32 {
        (elapsed / self.duration.max(1.0)).max(0.0)
    }

    fn extra_heroes(&self) -> f32 {
        if self.scale_with_heroes {
            self.heroes.saturating_sub(1) as f32
        } else {
            0.0
        }
    }

    /// The pool of roster indices at a point in the run, from a roster `tiers` long.
    pub fn wave_pool(&self, elapsed: f32, tiers: usize) -> [usize; 3] {
        // The roster spread over thirty steps of the run, the last three from step 28: on The
        // Barrens' thirty tiers, one newer every step.
        let step = (self.progress(elapsed) * 30.0) as usize;
        let span = tiers.saturating_sub(3);
        let lo = (step.saturating_sub(1) * span / 27).min(span);
        [lo, lo + 1, lo + 2]
    }

    /// How many enemies the spawner keeps alive at a point in the run: the stage's crowd, the
    /// wave's swell or lull on it, the density and the heroes.
    pub fn target_alive(&self, elapsed: f32) -> usize {
        let crowd = self.density * (1.0 + 0.35 * self.extra_heroes());
        let solo = (CROWD_START + self.progress(elapsed) * (CROWD_PEAK - CROWD_START) * 1.05)
            .min(CROWD_PEAK);
        ((solo * wave_crowd(elapsed) * crowd) as usize).clamp(1, HARD_CROWD_CAP)
    }

    /// Enemy health grows on top of the tiers, so a late kill takes several spells.
    pub fn hp_scale(&self, elapsed: f32) -> f32 {
        (1.0 + self.progress(elapsed) * 0.75) * self.health * (1.0 + 0.5 * self.extra_heroes())
    }

    /// Enemy damage grows the same way.
    pub fn damage_scale(&self, elapsed: f32) -> f32 {
        (1.0 + self.progress(elapsed) * HORDE_DAMAGE_GROWTH) * self.damage
    }

    /// When the `n`th boss (from 0) comes: its [`BOSS_TIMES`] slot, scaled to the run's length.
    pub fn boss_time(&self, n: usize) -> f32 {
        let slot = BOSS_TIMES[n.min(BOSS_TIMES.len() - 1)];
        slot * self.duration.max(1.0) / FINAL_BOSS_SECS
    }

    /// Bosses due by `elapsed`, the final boss among them once its time has come.
    pub fn bosses_due(&self, elapsed: f32) -> u32 {
        (0..BOSS_TIMES.len())
            .take_while(|&n| self.boss_time(n) <= elapsed)
            .count() as u32
    }

    /// The `n`th boss (from 0) of a roster, and whether it is the final one.
    pub fn boss<'a>(&self, roster: &'a Roster, n: u32) -> (&'a EnemyDef, bool) {
        let last = roster.bosses.len().saturating_sub(1);
        let n = (n as usize).min(last);
        (&roster.bosses[n], n == last)
    }
}

/// Whether a surge is on at `elapsed` seconds into a stage: the last [`SURGE_SECS`] of every
/// [`WAVE_EVERY`].
pub(crate) fn surging(elapsed: f32) -> bool {
    elapsed >= 0.0 && elapsed.rem_euclid(WAVE_EVERY) >= WAVE_EVERY - SURGE_SECS
}

/// The crowd's swell in a surge, or its lull between.
pub(crate) fn wave_crowd(elapsed: f32) -> f32 {
    if surging(elapsed) {
        SURGE_CROWD
    } else {
        LULL_CROWD
    }
}

/// Seconds to the next surge, 0 while one is on.
pub(crate) fn next_surge_in(elapsed: f32) -> f32 {
    if surging(elapsed) {
        0.0
    } else {
        (WAVE_EVERY - SURGE_SECS) - elapsed.max(0.0).rem_euclid(WAVE_EVERY)
    }
}

/// How far through a solo run, 0 at the start and 1 at [`FINAL_BOSS_SECS`].
pub(crate) fn progress(elapsed: f32) -> f32 {
    Rules::SOLO.progress(elapsed)
}

/// [`Rules::wave_pool`] under the solo rules.
pub(crate) fn wave_pool(elapsed: f32, tiers: usize) -> [usize; 3] {
    Rules::SOLO.wave_pool(elapsed, tiers)
}

/// [`Rules::target_alive`] under the solo rules.
pub(crate) fn target_alive(elapsed: f32) -> usize {
    Rules::SOLO.target_alive(elapsed)
}

/// [`Rules::hp_scale`] under the solo rules.
pub(crate) fn hp_scale(elapsed: f32) -> f32 {
    Rules::SOLO.hp_scale(elapsed)
}

/// [`Rules::damage_scale`] under the solo rules.
pub(crate) fn damage_scale(elapsed: f32) -> f32 {
    Rules::SOLO.damage_scale(elapsed)
}

/// The experience to go from `level` to the next: Vampire Survivors' curve, cheap early.
pub(crate) fn xp_to_next(level: u32) -> f32 {
    let l = level as f32;
    // The first levels come quick, so a run opens on choices; from 10 on the curve is as before.
    if level < 10 {
        5.0 + (l - 1.0) * 8.0
    } else if level < 20 {
        77.0 + (l - 10.0) * 12.0
    } else if level < 40 {
        195.0 + (l - 20.0) * 13.0
    } else {
        455.0 + (l - 40.0) * 16.0
    }
}

/// A creature on a battleground's roster: who it is, its pace and its size. Its health, damage
/// and experience come from its place on the roster, read off The Barrens' tuned ladder.
pub(crate) struct Foe {
    name: &'static str,
    display: u32,
    level: u32,
    speed: f32,
    /// The drawn scale, chosen against the model's Stand height so the field reads as The
    /// Barrens does: about 2-3.5 yd for the horde, 4-5 for the great beasts, 5-9 for a boss.
    scale: f32,
    big: bool,
}

const fn foe(name: &'static str, display: u32, level: u32, speed: f32, scale: f32) -> Foe {
    Foe {
        name,
        display,
        level,
        speed,
        scale,
        big: false,
    }
}

const fn giant(name: &'static str, display: u32, level: u32, speed: f32, scale: f32) -> Foe {
    Foe {
        big: true,
        ..foe(name, display, level, speed, scale)
    }
}

/// Where a run is fought.
pub(crate) struct MapDef {
    pub name: &'static str,
    /// The campaign's title for the zone, under its name on the main screen.
    pub zone: &'static str,
    /// The `Map.dbc` row.
    pub map: u32,
    /// Where the hero starts (WoW coordinates).
    pub center: [f32; 3],
    /// How far from the centre the hero may walk.
    pub leash: f32,
    /// The results screen's line on a win.
    pub victory: &'static str,
    /// The multiplier on the ladder's health and damage: the campaign hardens zone by zone.
    pub might: f32,
    /// Weakest first, numbered off The Barrens' tuned [`ENEMIES`].
    foes: &'static [Foe],
    /// One per [`BOSS_TIMES`] slot, in order, numbered off [`BOSSES`]; the last is the final
    /// boss, whose fall wins the run.
    bosses: &'static [Foe],
}

/// Whether the campaign's zone `i` is open: the first always, any other once the zone before it
/// has been won (`won` says, by zone name, whether a zone's final boss has fallen).
pub(crate) fn unlocked(i: usize, won: impl Fn(&str) -> bool) -> bool {
    i < MAPS.len() && (i == 0 || won(MAPS[i - 1].name))
}

/// What opens zone `i`, as the main screen words it: "Defeat Edwin VanCleef in Elwynn Forest".
pub(crate) fn unlock_hint(i: usize) -> Option<String> {
    let prev = MAPS.get(i.checked_sub(1)?)?;
    let last = prev.bosses.last()?;
    Some(format!("Defeat {} in {}", last.name, prev.name))
}

/// The campaign, in order: each zone opens once the one before it has fallen to its final boss
/// ([`unlocked`]). A creature whose own look the tables do not have yet wears a close one's
/// (`// look:` names it), so every display here is one the client already draws.
pub(crate) static MAPS: &[MapDef] = &[
    MapDef {
        name: "Elwynn Forest",
        zone: "The Defias Uprising",
        map: 0,
        // Open, level ground among Elwynn Forest's own creatures: no building or big prop near.
        center: [-9548.5, 115.0, 60.0],
        leash: 150.0,
        victory: "Edwin VanCleef has fallen; Elwynn Forest is safe once more.",
        might: 1.0,
        foes: &[
            // look: Kobold Tunneler
            foe("Kobold Vermin", 139, 2, 4.2, 1.0),
            foe("Young Wolf", 447, 2, 5.4, 1.0),
            foe("Murloc Streamrunner", 527, 4, 4.2, 1.0),
            // look: Kobold Miner
            foe("Kobold Geomancer", 373, 7, 3.8, 1.0),
            foe("Defias Thug", 5035, 8, 4.2, 1.15),
            foe("Riverpaw Brute", 384, 9, 4.2, 1.0),
            // look: Gray Forest Wolf
            foe("Prowler", 380, 9, 5.4, 1.0),
            foe("Defias Rogue Wizard", 2359, 10, 3.8, 1.1),
            // look: Defias Bandit
            foe("Defias Enforcer", 2357, 11, 4.2, 1.2),
        ],
        bosses: &[
            foe("Hogger", 384, 11, 4.2, 1.9),
            foe("Mother Fang", 2541, 12, 4.6, 1.9),
            // look: Dreadmaul Mauler, an ogre
            foe("Rhahk'Zor", 14401, 19, 4.2, 2.0),
            // look: Defias Highwayman
            foe("Mr. Smite", 2342, 20, 4.2, 2.1),
            // look: Thuros Lightfingers
            foe("Edwin VanCleef", 3341, 21, 4.6, 2.3),
        ],
    },
    MapDef {
        name: "Duskwood",
        zone: "The Curse of Darkshire",
        map: 0,
        // Open, level ground among Duskwood's own creatures: no building or big prop near.
        center: [-10051.8, -792.5, 36.5],
        leash: 150.0,
        victory: "Morbent Fel is no more; Duskwood's curse lifts.",
        might: 1.2,
        foes: &[
            foe("Pygmy Venom Web Spider", 958, 19, 4.6, 1.0),
            foe("Venom Web Spider", 955, 20, 4.6, 1.0),
            foe("Skeletal Warrior", 200, 22, 4.2, 1.2),
            foe("Green Recluse", 2541, 22, 4.6, 1.0),
            foe("Skeletal Mage", 9783, 23, 4.2, 1.0),
            foe("Young Black Ravager", 246, 24, 5.4, 1.0),
            foe("Skeletal Horror", 9786, 24, 3.6, 1.0),
            foe("Black Widow Hatchling", 368, 25, 4.6, 1.0),
            foe("Black Ravager", 741, 25, 5.4, 1.0),
            foe("Black Ravager Mastiff", 9562, 26, 5.4, 1.0),
            foe("Nightbane Worgen", 657, 27, 5.4, 1.0),
            foe("Nightbane Shadow Weaver", 203, 28, 5.4, 1.0),
            foe("Nightbane Dark Runner", 736, 29, 5.4, 1.0),
            foe("Nightbane Vile Fang", 522, 30, 5.4, 1.0),
            foe("Fetid Corpse", 559, 30, 3.6, 1.3),
            foe("Nightbane Tainted One", 1098, 31, 5.4, 1.0),
        ],
        bosses: &[
            foe("Lupos", 11412, 23, 5.4, 1.9),
            foe("Naraxis", 963, 27, 4.6, 1.9),
            foe("Fenros", 11179, 32, 5.4, 1.9),
            foe("Nefaru", 11181, 34, 5.4, 1.9),
            // look: Necromancer
            foe("Morbent Fel", 10427, 32, 4.2, 2.1),
        ],
    },
    MapDef {
        name: "Warsong Gulch",
        zone: "The Legion's Gulch",
        map: 489,
        // Midfield, between the two flag rooms.
        center: [1230.0, 1460.0, 312.0],
        leash: 60.0,
        victory: "Lord Kazzak is banished from the gulch.",
        might: 1.4,
        foes: &[
            foe("Starving Snickerfang", 8050, 45, 4.6, 1.0),
            foe("Snickerfang Hyena", 2714, 49, 5.0, 1.15),
            foe("Black Slayer", 10824, 47, 5.0, 0.6),
            foe("Redstone Crystalhide", 798, 51, 3.6, 1.3),
            foe("Shadowsworn Cultist", 7838, 51, 3.8, 1.3),
            foe("Felbeast", 7949, 50, 4.8, 0.85),
            foe("Felhound", 1913, 54, 5.2, 1.0),
            foe("Felguard Sentry", 9017, 55, 4.2, 0.9),
            foe("Manahound", 6173, 60, 5.6, 1.15),
            giant("Lesser Infernal", 10906, 55, 3.4, 1.0),
            foe("Felguard Elite", 7970, 61, 4.4, 1.0),
            giant("Doomguard Commander", 4426, 61, 4.0, 1.1),
        ],
        bosses: &[
            foe("Akubar the Seer", 10920, 54, 4.4, 2.6),
            foe("Lord Azrethoc", 4426, 55, 4.6, 1.35),
            giant("Teremus the Devourer", 6378, 60, 5.0, 0.55),
            foe("Lord Hel'nurath", 14556, 62, 4.8, 1.65),
            giant("Lord Kazzak", 12449, 63, 4.6, 0.75),
        ],
    },
    MapDef {
        name: "Un'Goro Crater",
        zone: "The Primal Crater",
        map: 1,
        // Fire Plume Ridge's summit plateau, its lava lake on the west rim; the crater floor's
        // canopy would hide the fight from above.
        center: [-7136.0, -1309.0, -184.0],
        leash: 32.0,
        victory: "Even King Mosh has fallen.",
        might: 1.6,
        foes: &[
            foe("Bloodpetal Lasher", 11634, 49, 3.6, 0.9),
            foe("Young Diemetradon", 8510, 48, 4.4, 0.55),
            foe("Ravasaur", 5242, 50, 5.4, 0.75),
            foe("Muculent Ooze", 11140, 50, 3.2, 1.4),
            foe("Ravasaur Hunter", 5292, 51, 5.6, 0.85),
            foe("Tar Beast", 1549, 51, 3.4, 1.0),
            foe("Bloodpetal Flayer", 11635, 52, 3.8, 1.05),
            foe("Primal Ooze", 4754, 52, 3.4, 1.6),
            foe("Gorishi Wasp", 11090, 53, 5.4, 0.55),
            foe("Pterrordax", 8411, 53, 5.2, 0.5),
            foe("Diemetradon", 8511, 52, 4.6, 0.7),
            foe("Glutinous Ooze", 1146, 54, 3.6, 1.8),
            foe("Tar Lurker", 9010, 54, 3.6, 1.15),
            foe("Scorching Elemental", 1070, 54, 4.2, 1.05),
            foe("Frenzied Pterrordax", 8412, 55, 5.6, 0.55),
            foe("Elder Diemetradon", 8512, 55, 4.6, 0.8),
            giant("Stegodon", 5241, 52, 3.8, 0.72),
            giant("Stone Guardian", 8395, 52, 3.6, 1.45),
            giant("Plated Stegodon", 5287, 54, 3.8, 0.78),
            giant("Devilsaur", 5239, 56, 5.0, 0.72),
        ],
        bosses: &[
            foe("Ravasaur Matriarch", 11319, 56, 5.6, 1.55),
            foe("Uhk'loc", 8129, 59, 4.8, 2.4),
            foe("Baron Charr", 14517, 58, 4.6, 1.95),
            giant("Tyrant Devilsaur", 5240, 60, 5.0, 1.0),
            giant("King Mosh", 5305, 61, 5.0, 1.2),
        ],
    },
    MapDef {
        name: "Arathi Basin",
        zone: "The Syndicate's Claim",
        map: 529,
        // The Blacksmith, the basin's middle.
        center: [977.0, 1046.0, -44.0],
        leash: 70.0,
        victory: "Lord Aliden Perenolde's claim on Arathi is broken.",
        might: 1.8,
        foes: &[
            // look: Defias Trapper
            foe("Syndicate Spy", 2331, 30, 4.2, 1.1),
            // look: Razzashi Raptor
            foe("Highland Strider", 2571, 31, 5.6, 0.9),
            // look: Defias Highwayman
            foe("Syndicate Highwayman", 2342, 32, 4.2, 1.15),
            // look: Redridge Brute, a gnoll
            foe("Mudsnout Gnoll", 500, 32, 4.2, 1.0),
            // look: Gurubashi Axe Thrower
            foe("Witherbark Axe Thrower", 11074, 34, 4.2, 1.1),
            // look: Blackrock Hunter
            foe("Defiler Scout", 6031, 35, 4.2, 1.0),
            // look: Dreadmaul Mauler
            foe("Boulderfist Ogre", 14401, 36, 3.6, 1.1),
            // look: Blackrock Summoner
            foe("Defiler Shaman", 6030, 37, 3.8, 1.0),
            // look: Harvest Golem
            giant("Arathor Farm Golem", 367, 37, 3.6, 1.1),
            // look: Blackrock Champion
            foe("Defiler Elite", 6044, 39, 4.2, 1.1),
            // look: Dreadmaul Mauler
            giant("Boulderfist Enforcer", 14401, 40, 3.6, 1.3),
        ],
        bosses: &[
            // look: Gurubashi Axe Thrower
            foe("Nimar the Slayer", 11074, 37, 4.6, 2.0),
            // look: Dreadmaul Mauler
            foe("Kovork", 14401, 38, 4.2, 1.9),
            // look: Klaven Mortwake
            foe("Marshal Redpath", 5805, 40, 4.2, 2.18),
            // look: Dreadmaul Mauler
            foe("Molok the Crusher", 14401, 41, 4.2, 2.2),
            // look: Brainwashed Noble
            foe("Lord Aliden Perenolde", 3267, 42, 4.4, 2.3),
        ],
    },
    MapDef {
        name: "Feralas",
        zone: "The Gordok Tribute",
        map: 1,
        // The wilds south of Camp Mojache.
        center: [-4500.0, 150.0, 30.0],
        leash: 80.0,
        victory: "King Gordok's tribute is paid in full.",
        might: 2.0,
        foes: &[
            // look: Rot Hide Mongrel
            foe("Woodpaw Mongrel", 847, 40, 4.2, 1.0),
            // look: Riverpaw Bandit
            foe("Woodpaw Trapper", 383, 41, 4.2, 1.0),
            // look: Riverpaw Mystic
            foe("Woodpaw Mystic", 502, 42, 3.8, 1.0),
            // look: Wendigo
            foe("Rage Scar Yeti", 950, 43, 4.2, 1.1),
            // look: Jadefire Rogue, a satyr
            foe("Wildspawn Rogue", 8575, 44, 5.2, 1.0),
            // look: Riverpaw Brute
            foe("Woodpaw Alpha", 384, 45, 4.2, 1.1),
            // look: Dreadmaul Mauler
            foe("Gordunni Ogre", 14401, 45, 3.6, 1.1),
            // look: Jadefire Felsworn
            foe("Wildspawn Felsworn", 11334, 46, 4.8, 1.1),
            // look: Wendigo
            foe("Feral Scar Yeti", 950, 47, 4.2, 1.25),
            // look: Ice Claw Bear
            giant("Ironfur Bear", 8840, 47, 4.6, 1.1),
            // look: Dreadmaul Mauler
            giant("Gordunni Mauler", 14401, 48, 3.6, 1.3),
        ],
        bosses: &[
            // look: Ice Claw Bear
            giant("Old Grizzlegut", 8840, 46, 4.6, 1.9),
            // look: Jadefire Felsworn
            foe("Zevrim Thornhoof", 11334, 57, 4.8, 2.0),
            // look: Oakenscowl
            foe("Tendris Warpwood", 2567, 60, 4.2, 2.6),
            // look: Dreadmaul Mauler
            foe("Captain Kromcrush", 14401, 61, 4.2, 2.2),
            // look: Dreadmaul Mauler
            foe("King Gordok", 14401, 62, 4.4, 2.6),
        ],
    },
    MapDef {
        name: "Gurubashi Arena",
        zone: "The Blood God's Arena",
        map: 0,
        // The arena floor; the stands ring it.
        center: [-13205.0, 272.0, 22.0],
        leash: 30.0,
        victory: "Hakkar the Soulflayer is cast out; the arena roars your name.",
        might: 2.2,
        foes: &[
            foe("Shadowmaw Panther", 11452, 37, 5.6, 1.4),
            foe("Cold Eye Basilisk", 8797, 39, 3.8, 1.0),
            foe("Jungle Stalker", 11317, 40, 5.2, 0.8),
            foe("Elder Mistvale Gorilla", 838, 40, 4.0, 1.1),
            foe("Zanzil Zombie", 1065, 43, 3.4, 1.1),
            foe("Thrashtail Basilisk", 8802, 41, 4.0, 1.15),
            foe("Bloodsail Sea Dog", 796, 44, 4.0, 1.05),
            foe("Gurubashi Axe Thrower", 11074, 60, 4.2, 1.1),
            foe("Razzashi Serpent", 15182, 60, 4.8, 0.85),
            foe("Zulian Panther", 633, 60, 5.6, 1.6),
            foe("Gurubashi Headhunter", 11109, 60, 4.4, 1.15),
            foe("Bloodseeker Bat", 14562, 60, 5.6, 0.55),
            foe("Razzashi Raptor", 2571, 60, 5.6, 0.95),
            foe("Gurubashi Blood Drinker", 11080, 60, 4.2, 1.2),
            foe("Zulian Tiger", 11031, 61, 5.4, 1.8),
            foe("Hakkari Shadowcaster", 11229, 61, 4.0, 1.2),
            foe("Gurubashi Champion", 11100, 61, 4.4, 1.3),
            foe("Gurubashi Bat Rider", 15303, 62, 5.0, 0.6),
            giant("Gurubashi Berserker", 14832, 62, 4.0, 1.7),
        ],
        bosses: &[
            giant("King Bangalash", 616, 43, 5.8, 3.0),
            foe("Bloodlord Mandokir", 11288, 63, 5.0, 2.1),
            foe("High Priest Thekal", 15216, 63, 5.0, 2.3),
            foe("Jin'do the Hexxer", 11311, 63, 4.6, 2.5),
            giant("Hakkar the Soulflayer", 15295, 63, 4.4, 2.7),
        ],
    },
    MapDef {
        name: "Scarlet Monastery",
        zone: "The Scarlet Crusade",
        map: 0,
        // The fields below the monastery's walls, in Tirisfal Glades.
        center: [2800.0, -680.0, 130.0],
        leash: 50.0,
        victory: "High Inquisitor Whitemane's crusade ends at her own gates.",
        might: 2.4,
        foes: &[
            foe("Rattlecage Soldier", 200, 30, 4.2, 1.2),
            foe("Rotting Dead", 1200, 30, 3.6, 1.0),
            foe("Vampiric Duskbat", 8808, 31, 5.2, 0.8),
            // look: Scarlet Missionary
            foe("Scarlet Scout", 2468, 32, 4.2, 1.15),
            foe("Darkeye Bonecaster", 11398, 32, 3.8, 1.0),
            foe("Cursed Darkhound", 9020, 33, 4.2, 1.4),
            // look: Scarlet Zealot
            foe("Scarlet Myrmidon", 2483, 34, 4.2, 1.15),
            foe("Hungering Dead", 1202, 34, 3.6, 1.0),
            // look: Scarlet Missionary
            foe("Scarlet Adept", 2468, 35, 3.8, 1.15),
            foe("Shambling Horror", 1197, 36, 3.6, 1.1),
            // look: Scarlet Zealot
            foe("Scarlet Champion", 2483, 37, 4.4, 1.3),
        ],
        bosses: &[
            // look: Scarlet Zealot
            foe("Interrogator Vishas", 2483, 32, 4.2, 2.0),
            // look: Cursed Darkhound
            foe("Houndmaster Loksey", 9020, 34, 5.0, 2.4),
            // look: Scarlet Missionary
            foe("Arcanist Doan", 2468, 37, 4.0, 2.1),
            // look: Scarlet Zealot
            foe("Herod", 2483, 40, 4.4, 2.4),
            // look: Scarlet Missionary
            foe("High Inquisitor Whitemane", 2468, 42, 4.4, 2.3),
        ],
    },
    MapDef {
        name: "Alterac Valley",
        zone: "The Frozen Front",
        map: 30,
        // Snowfall, the valley's middle.
        center: [-202.0, -112.0, 78.0],
        leash: 70.0,
        victory: "Azuregos takes wing from the valley.",
        might: 2.6,
        foes: &[
            // look: Winter Wolf
            foe("Frostwolf", 785, 51, 5.4, 1.1),
            // look: Rot Hide Gnoll
            foe("Wildpaw Gnoll", 10852, 51, 4.2, 1.0),
            // look: Frostmane Snowstrider
            foe("Winterax Sentry", 5607, 52, 4.2, 1.2),
            // look: Wendigo
            foe("Alterac Yeti", 950, 53, 4.2, 1.25),
            // look: Frostmane Seer
            foe("Winterax Seer", 5610, 54, 3.8, 1.2),
            // look: Blackrock Tracker
            foe("Frostwolf Bloodhound", 6040, 55, 4.6, 1.0),
            // look: Blackrock Gladiator
            foe("Frostwolf Guardian", 6042, 56, 4.2, 1.1),
            // look: Ice Claw Bear
            giant("Alterac Bear", 8840, 57, 4.6, 1.15),
            // look: Blackrock Champion
            foe("Frostwolf Champion", 6044, 58, 4.2, 1.2),
        ],
        bosses: &[
            // look: Frostmane Seer
            foe("Korrak the Bloodrager", 5610, 59, 4.6, 2.2),
            foe("Great Father Arctikus", 5625, 60, 4.2, 2.28),
            // look: Oakenscowl
            foe("Ivus the Forest Lord", 2567, 61, 4.2, 2.8),
            // look: Blackrock Summoner
            foe("Drek'Thar", 6030, 62, 4.0, 2.3),
            // look: Ysondre
            giant("Azuregos", 15364, 63, 4.8, 0.58),
        ],
    },
    MapDef {
        name: "Blackrock Mountain",
        zone: "The Black Dragonflight",
        map: 0,
        // The Burning Steppes under the mountain.
        center: [-7600.0, -1900.0, 135.0],
        leash: 80.0,
        victory: "Nefarian falls, and the mountain with him.",
        might: 2.8,
        foes: &[
            foe("Black Dragon Whelp", 387, 52, 4.2, 1.0),
            foe("Blackrock Hunter", 6031, 53, 4.2, 1.1),
            foe("Blackrock Summoner", 6030, 54, 3.8, 1.1),
            foe("Blackrock Tracker", 6040, 55, 4.2, 1.1),
            foe("Scorching Elemental", 1070, 55, 4.2, 1.05),
            foe("Blackrock Gladiator", 6042, 56, 4.2, 1.15),
            foe("Black Slayer", 10824, 57, 5.0, 0.7),
            foe("Blackrock Champion", 6044, 58, 4.2, 1.2),
            // look: Lesser Infernal
            giant("Molten Giant", 10906, 60, 3.4, 1.2),
        ],
        bosses: &[
            // look: Dreadmaul Mauler
            foe("Highlord Omokk", 14401, 59, 4.2, 2.3),
            // look: Gurubashi Axe Thrower
            foe("War Master Voone", 11074, 59, 4.6, 2.0),
            // look: Scorching Elemental
            foe("Pyroguard Emberseer", 1070, 60, 4.4, 2.4),
            // look: Blackrock Champion
            foe("Overlord Wyrmthalak", 6044, 61, 4.4, 2.3),
            // look: Black Dragon Whelp
            giant("Nefarian", 387, 63, 5.0, 3.5),
        ],
    },
    MapDef {
        name: "Gates of Ahn'Qiraj",
        zone: "The War of the Shifting Sands",
        map: 1,
        // The sands before the Scarab Wall.
        center: [-8180.0, 1536.0, 5.0],
        leash: 80.0,
        victory: "C'Thun's eye closes; the Qiraji retreat behind the Scarab Wall.",
        might: 3.0,
        foes: &[
            foe("Sand Skitterer", 8014, 56, 5.0, 1.0),
            foe("Stonelash Scorpid", 15383, 55, 4.6, 1.6),
            foe("Dredge Striker", 15386, 56, 4.4, 1.0),
            foe("Twilight Avenger", 11811, 58, 4.0, 1.1),
            foe("Hive'Ashi Drone", 2303, 58, 5.4, 0.55),
            foe("Stonelash Flayer", 15385, 57, 4.8, 1.75),
            foe("Hive'Ashi Defender", 12153, 59, 4.2, 1.3),
            foe("Dust Stormer", 8715, 57, 4.6, 1.05),
            foe("Twilight Stonecaller", 11815, 59, 3.8, 1.1),
            foe("Hive'Zora Wasp", 11088, 59, 5.6, 0.6),
            foe("Hive'Zora Reaver", 11085, 60, 4.8, 1.0),
            foe("Hive'Regal Ambusher", 11106, 60, 5.2, 1.05),
            foe("Vekniss Wasp", 15335, 60, 5.6, 0.65),
            foe("Qiraji Gladiator", 15741, 61, 4.4, 0.75),
            foe("Vekniss Warrior", 15334, 61, 4.6, 1.15),
            giant("Obsidian Destroyer", 15343, 61, 3.8, 1.2),
            giant("Qiraji Champion", 15340, 63, 4.6, 0.95),
            giant("Anubisath Sentinel", 15347, 61, 4.0, 0.62),
        ],
        bosses: &[
            giant("Kurinnaxx", 15742, 62, 5.0, 1.15),
            foe("General Rajaxx", 15376, 63, 4.8, 1.15),
            foe("Moam", 15392, 63, 4.2, 1.6),
            foe("Ossirian the Unscarred", 15432, 63, 4.6, 0.82),
            // look: Buru the Gorger
            giant("C'Thun", 15654, 63, 4.0, 1.1),
        ],
    },
    MapDef {
        name: "Naxxramas",
        zone: "The Necropolis",
        map: 0,
        // Plaguewood, under the necropolis.
        center: [3010.0, -3790.0, 124.0],
        leash: 80.0,
        victory: "Kel'Thuzad's necropolis falls dark.",
        might: 3.2,
        foes: &[
            foe("Scourge Soldier", 5231, 55, 4.4, 1.15),
            foe("Plagued Swine", 6121, 53, 5.0, 1.0),
            foe("Cannibal Ghoul", 10626, 56, 4.6, 1.3),
            foe("Cursed Mage", 11397, 55, 3.6, 0.95),
            foe("Putrid Gargoyle", 7854, 56, 5.2, 0.85),
            foe("Scourge Warder", 612, 56, 4.4, 1.2),
            foe("Death Cultist", 10391, 56, 3.8, 1.1),
            foe("Frenzied Plaguehound", 9022, 57, 5.6, 1.4),
            foe("Torn Screamer", 984, 57, 5.0, 0.7),
            foe("Necromancer", 10427, 57, 3.6, 1.15),
            foe("Carrion Devourer", 7900, 57, 4.0, 1.25),
            foe("Crypt Walker", 11178, 58, 4.6, 0.85),
            foe("Putrid Shrieker", 7855, 58, 5.4, 1.0),
            foe("Hate Shrieker", 10751, 58, 5.0, 0.8),
            foe("Ziggurat Protector", 10627, 59, 4.4, 1.6),
            foe("Deathknight", 16508, 60, 4.6, 1.75),
            giant("Stitched Horror", 1693, 58, 3.6, 1.25),
            giant("Slaughterhouse Protector", 7864, 60, 3.6, 1.3),
            giant("Death Lord", 10729, 61, 5.0, 1.05),
        ],
        bosses: &[
            giant("Anub'Rekhan", 15931, 63, 4.6, 0.55),
            foe("Grand Widow Faerlina", 15940, 63, 4.8, 2.9),
            giant("Patchwerk", 16174, 63, 4.0, 1.7),
            giant("Thaddius", 16137, 63, 4.0, 0.98),
            foe("Kel'Thuzad", 15945, 63, 4.4, 2.1),
        ],
    },
];

/// Where a piece of gear is worn: the ten slots of the main screen's Equipment panel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum GearSlot {
    Head,
    Shoulders,
    Chest,
    Wrists,
    Hands,
    Waist,
    Legs,
    Feet,
    MainHand,
    OffHand,
}

impl GearSlot {
    pub const ALL: [GearSlot; 10] = [
        GearSlot::Head,
        GearSlot::Shoulders,
        GearSlot::Chest,
        GearSlot::Wrists,
        GearSlot::Hands,
        GearSlot::Waist,
        GearSlot::Legs,
        GearSlot::Feet,
        GearSlot::MainHand,
        GearSlot::OffHand,
    ];

    /// The slot's name, as the screen and the profile write it.
    pub const fn name(self) -> &'static str {
        match self {
            GearSlot::Head => "Head",
            GearSlot::Shoulders => "Shoulders",
            GearSlot::Chest => "Chest",
            GearSlot::Wrists => "Wrists",
            GearSlot::Hands => "Hands",
            GearSlot::Waist => "Waist",
            GearSlot::Legs => "Legs",
            GearSlot::Feet => "Feet",
            GearSlot::MainHand => "Main Hand",
            GearSlot::OffHand => "Off Hand",
        }
    }

    /// The word an item of this slot is named with: "Hogger's Gauntlets".
    const fn noun(self) -> &'static str {
        match self {
            GearSlot::Head => "Helm",
            GearSlot::Shoulders => "Mantle",
            GearSlot::Chest => "Chestguard",
            GearSlot::Wrists => "Bracers",
            GearSlot::Hands => "Gauntlets",
            GearSlot::Waist => "Girdle",
            GearSlot::Legs => "Legguards",
            GearSlot::Feet => "Boots",
            GearSlot::MainHand => "Edge",
            GearSlot::OffHand => "Ward",
        }
    }

    /// What the slot's gear gives at rarity 1 in the first zone; everything else scales it.
    const fn bonus(self) -> Bonus {
        match self {
            GearSlot::Head => Bonus::pct(Stat::MaxHp, 0.04),
            GearSlot::Shoulders => Bonus::flat(Stat::Area, 0.03),
            GearSlot::Chest => Bonus::flat(Stat::Armor, 1.5),
            GearSlot::Wrists => Bonus::flat(Stat::Haste, 0.02),
            GearSlot::Hands => Bonus::flat(Stat::Crit, 0.01),
            GearSlot::Waist => Bonus::pct(Stat::Magnet, 0.06),
            GearSlot::Legs => Bonus::flat(Stat::MaxHp, 10.0),
            GearSlot::Feet => Bonus::pct(Stat::Speed, 0.03),
            GearSlot::MainHand => Bonus::flat(Stat::Might, 0.05),
            GearSlot::OffHand => Bonus::flat(Stat::DamageReduction, 0.015),
        }
    }
}

/// An item's quality, WoW's colours.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) enum Rarity {
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const fn name(self) -> &'static str {
        match self {
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }

    /// The quality colour, `0xRRGGBB`.
    pub const fn color(self) -> u32 {
        match self {
            Rarity::Uncommon => 0x1eff00,
            Rarity::Rare => 0x0070dd,
            Rarity::Epic => 0xa335ee,
            Rarity::Legendary => 0xff8000,
        }
    }

    const fn power(self) -> f32 {
        match self {
            Rarity::Uncommon => 1.0,
            Rarity::Rare => 1.5,
            Rarity::Epic => 2.2,
            Rarity::Legendary => 3.2,
        }
    }
}

/// A piece of gear: dropped by one boss of one zone, worn in one slot, kept between runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ItemDef {
    /// "Hogger's Helm": the profile stores items by it.
    pub name: String,
    pub slot: GearSlot,
    pub rarity: Rarity,
    /// The zone ([`MAPS`] index) and boss slot that drop it.
    pub zone: usize,
    pub boss: usize,
    /// What wearing it brings into a run.
    pub bonuses: Vec<Bonus>,
}

/// One in this many enemies of the horde drops one of the zone's items; a boss always drops its
/// own.
pub(crate) const HORDE_ITEM_ODDS: u32 = 600;

/// Every item, five per zone: one per boss, in the campaign's order. The slots turn through all
/// ten over each two zones, the bosses' items grow rarer toward the final one, and every zone's
/// numbers grow with its place in the campaign.
pub(crate) static ITEMS: std::sync::LazyLock<Vec<ItemDef>> = std::sync::LazyLock::new(|| {
    let mut out = Vec::new();
    for (z, m) in MAPS.iter().enumerate() {
        for (k, boss) in m.bosses.iter().enumerate() {
            let slot = GearSlot::ALL[(z * m.bosses.len() + k) % GearSlot::ALL.len()];
            let rarity = match k {
                0 | 1 => Rarity::Uncommon,
                2 | 3 => Rarity::Rare,
                _ if z + 2 >= MAPS.len() => Rarity::Legendary,
                _ => Rarity::Epic,
            };
            let scale = rarity.power() * (1.0 + 0.12 * z as f32);
            let main = slot.bonus();
            let mut bonuses = vec![Bonus {
                stat: main.stat,
                amount: match main.amount {
                    Amount::Flat(v) => Amount::Flat(v * scale),
                    Amount::Percent(v) => Amount::Percent(v * scale),
                },
            }];
            // An epic or legendary piece mends its wearer too.
            if rarity >= Rarity::Epic {
                bonuses.push(Bonus::flat(Stat::Regen, 0.3 * scale));
            }
            out.push(ItemDef {
                name: format!("{}'s {}", boss.name, slot.noun()),
                slot,
                rarity,
                zone: z,
                boss: k,
                bonuses,
            });
        }
    }
    out
});

// ── The run's own economy: gold, chests, the field's elites and the run's items ──────────────

/// One in this many horde kills drops a coin; a boss drops a pile, an elite a purse.
pub(crate) const COIN_ODDS: u32 = 3;
/// A boss's pile of gold and an elite's purse, before the gold stat.
pub(crate) const BOSS_GOLD: u32 = 25;
pub(crate) const ELITE_GOLD: u32 = 10;

/// What the next chest costs, `opened` chests into the run: 20 gold, each one dearer by 40%.
pub(crate) fn chest_cost(opened: u32) -> u32 {
    ((20.0 * 1.4f32.powi(opened.min(40) as i32)) / 5.0).round() as u32 * 5
}

/// Chests on the field at once, and the seconds an opened one takes to come back elsewhere.
pub(crate) const CHESTS: usize = 6;
pub(crate) const CHEST_RESPAWN_SECS: f32 = 40.0;
/// Seconds a hero stands at a chest to open it, and how near.
pub(crate) const CHEST_OPEN_SECS: f32 = 1.0;
pub(crate) const CHEST_REACH: f32 = 3.0;

/// The half of the gold left unspent at the end that the profile keeps.
pub(crate) const UNSPENT_GOLD_KEPT: f32 = 0.5;

/// How likely an elite drops a key, which opens a chest for nothing.
pub(crate) const KEY_ODDS: f32 = 0.4;
/// The most keys a hero carries.
pub(crate) const MAX_KEYS: u8 = 9;

/// Jars and crates standing about the field, how near a hero breaks one, and how long a broken
/// one takes to stand again elsewhere.
pub(crate) const BREAKABLES: usize = 16;
pub(crate) const BREAK_REACH: f32 = 1.8;
pub(crate) const BREAKABLE_RESPAWN_SECS: f32 = 45.0;
/// What a broken jar holds: gold, a few coins' worth.
pub(crate) const JAR_GOLD: (u32, u32) = (2, 6);
/// What a broken crate holds: a heal half the time, a coin otherwise.
pub(crate) const CRATE_HEAL: f32 = 40.0;
pub(crate) const CRATE_HEAL_ODDS: f32 = 0.5;

/// The shady merchant: what it sells at a time, how near a hero trades, and how often it restocks.
pub(crate) const SHOP_SLOTS: usize = 3;
pub(crate) const MERCHANT_REACH: f32 = 5.0;
pub(crate) const SHOP_RESTOCK_SECS: f32 = 120.0;
/// How much dearer each item is than the one bought before it.
pub(crate) const SHOP_MARKUP: f32 = 1.15;
/// The merchant's icon (Pick Lock, a `Spell.dbc` row for the icon only) and its marks' colour.
pub(crate) const MERCHANT_ICON: u32 = 1804;
pub(crate) const MERCHANT_COLOR: [f32; 3] = [0.35, 0.95, 0.75];

/// What a run item costs at the merchant, `bought` items into the run: its rarity's price, a
/// markup for each bought before, to the nearest five.
pub(crate) fn shop_price(rarity: CardRarity, bought: u32) -> u32 {
    let base = match rarity {
        CardRarity::Common => 30.0,
        CardRarity::Uncommon => 45.0,
        CardRarity::Rare => 70.0,
        CardRarity::Epic => 110.0,
        CardRarity::Legendary => 170.0,
    };
    let p = base * SHOP_MARKUP.powi(bought.min(60) as i32);
    ((p / 5.0).round() * 5.0) as u32
}

/// A run item: picked up in the run, gone at its end, stacking.
#[derive(Debug, PartialEq)]
pub(crate) struct RunItemDef {
    pub key: &'static str,
    pub name: &'static str,
    /// Icon, a `Spell.dbc` row.
    pub spell: u32,
    /// What each stack gives.
    pub bonus: Bonus,
    /// How rare it is: the rarer, the less often it is drawn ([`CardRarity::item_odds`]).
    pub rarity: CardRarity,
    /// What it does beyond its stat, the simulation's to fire.
    pub effect: Option<ItemEffect>,
}

/// What a run item does beyond a stat, each number per stack. Only solo and the host fire them
/// (`abilities::apply_hits`, `enemies::move_enemies`), and their visuals go through `units::Fx`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ItemEffect {
    /// Every hit heals the hero this share of its damage.
    Lifesteal(f32),
    /// Every hit may arc to `jumps` more foes nearby for `damage` (times might), at `chance`;
    /// `spell` names the arc's visual.
    Chain {
        chance: f32,
        damage: f32,
        jumps: u8,
        spell: u32,
    },
    /// A blow taken returns this much damage (times might) to whoever struck.
    Thorns(f32),
    /// A kill may burst for `damage` (times might) on everything within `radius`, at `chance`;
    /// `spell` names the burst's visual.
    Burst {
        chance: f32,
        damage: f32,
        radius: f32,
        spell: u32,
    },
    /// A kill heals the hero this much.
    KillHeal(f32),
    /// Every hit may set its foe burning for `dps` (times might) a second over `secs`, at
    /// `chance`; `spell` names the burn on the damage meter and its visual.
    Ignite {
        chance: f32,
        dps: f32,
        secs: f32,
        spell: u32,
    },
    /// Every hit may chill its foe to `slow` of its speed for `secs`, at `chance`.
    Chill { chance: f32, slow: f32, secs: f32 },
    /// A hit that leaves a foe that is no boss under this share of its health finishes it, up
    /// to [`MAX_EXECUTE`].
    Execute(f32),
    /// A blow taken is dodged at this chance, all items together up to [`MAX_DODGE`].
    Dodge(f32),
    /// A kill wraps the hero in an absorb of `amount`, up to `max` at once, for
    /// [`KILL_SHIELD_SECS`].
    KillShield { amount: f32, max: f32 },
    /// Every `every` seconds a nova strikes everything within `radius` of the hero for `damage`
    /// (times might); `spell` names its visual.
    Nova {
        every: f32,
        damage: f32,
        radius: f32,
        spell: u32,
    },
    /// A kill may pay `gold` more (times the gold stat), at `chance`.
    Bounty { chance: f32, gold: f32 },
}

/// The most of its health a hit may leave a foe with and still finish it.
pub(crate) const MAX_EXECUTE: f32 = 0.25;
/// The most likely a blow is dodged, however many items.
pub(crate) const MAX_DODGE: f32 = 0.35;
/// How long a kill's absorb lasts.
pub(crate) const KILL_SHIELD_SECS: f32 = 6.0;
/// The most foes one nova strikes.
pub(crate) const MAX_NOVA_HITS: usize = 40;

/// The most likely an item's chance grows to, however many stacks.
pub(crate) const MAX_PROC_CHANCE: f32 = 0.6;
/// How far an item's arc looks for its next foe.
pub(crate) const ITEM_CHAIN_REACH: f32 = 9.0;

impl ItemEffect {
    /// The effect at `stacks`: amounts grow with each stack, a chance up to [`MAX_PROC_CHANCE`].
    pub fn at(self, stacks: u8) -> ItemEffect {
        let k = f32::from(stacks.max(1));
        let chance = |c: f32| (c * k).min(MAX_PROC_CHANCE);
        match self {
            ItemEffect::Lifesteal(f) => ItemEffect::Lifesteal(f * k),
            ItemEffect::Chain {
                chance: c,
                damage,
                jumps,
                spell,
            } => ItemEffect::Chain {
                chance: chance(c),
                damage: damage * k,
                jumps,
                spell,
            },
            ItemEffect::Thorns(d) => ItemEffect::Thorns(d * k),
            ItemEffect::Burst {
                chance: c,
                damage,
                radius,
                spell,
            } => ItemEffect::Burst {
                chance: chance(c),
                damage: damage * k,
                radius,
                spell,
            },
            ItemEffect::KillHeal(h) => ItemEffect::KillHeal(h * k),
            ItemEffect::Ignite {
                chance: c,
                dps,
                secs,
                spell,
            } => ItemEffect::Ignite {
                chance: chance(c),
                dps: dps * k,
                secs,
                spell,
            },
            ItemEffect::Chill {
                chance: c,
                slow,
                secs,
            } => ItemEffect::Chill {
                chance: chance(c),
                slow,
                secs,
            },
            ItemEffect::Execute(t) => ItemEffect::Execute((t * k).min(MAX_EXECUTE)),
            ItemEffect::Dodge(c) => ItemEffect::Dodge((c * k).min(MAX_DODGE)),
            ItemEffect::KillShield { amount, max } => ItemEffect::KillShield {
                amount: amount * k,
                max: max * k,
            },
            // A nova grows harder with each stack, and a little quicker.
            ItemEffect::Nova {
                every,
                damage,
                radius,
                spell,
            } => ItemEffect::Nova {
                every: every / k.powf(0.3),
                damage: damage * k,
                radius,
                spell,
            },
            ItemEffect::Bounty { chance: c, gold } => ItemEffect::Bounty {
                chance: chance(c),
                gold: gold * k,
            },
        }
    }
}

/// The most of one run item a hero carries.
pub(crate) const MAX_RUN_ITEM_STACKS: u8 = 9;
/// The most kinds of run item a hero carries: the character sheet's neck, two rings and two
/// trinkets. Once full, a run item is drawn among the kinds held.
pub(crate) const MAX_RUN_ITEM_KINDS: usize = 5;
/// The gold a run item gives instead when every kind held is at its most stacks.
pub(crate) const FULL_RUN_ITEMS_GOLD: u32 = 25;

pub(crate) static RUN_ITEMS: &[RunItemDef] = &[
    RunItemDef {
        key: "battle_standard",
        name: "Battle Standard",
        spell: 23880,
        bonus: Bonus::flat(Stat::Might, 0.06),
        rarity: CardRarity::Uncommon,
        effect: None,
    },
    RunItemDef {
        key: "hourglass",
        name: "Bronze Hourglass",
        spell: 25771,
        bonus: Bonus::flat(Stat::Haste, 0.04),
        rarity: CardRarity::Uncommon,
        effect: None,
    },
    RunItemDef {
        key: "eagle_eye",
        name: "Eagle Eye Lens",
        spell: 6197,
        bonus: Bonus::flat(Stat::Crit, 0.03),
        rarity: CardRarity::Rare,
        effect: None,
    },
    RunItemDef {
        key: "thick_hide",
        name: "Thick Hide",
        spell: 9116,
        bonus: Bonus::flat(Stat::Armor, 2.0),
        rarity: CardRarity::Common,
        effect: None,
    },
    RunItemDef {
        key: "troll_blood",
        name: "Troll's Blood",
        spell: 20555,
        bonus: Bonus::flat(Stat::Regen, 1.0),
        rarity: CardRarity::Common,
        effect: None,
    },
    RunItemDef {
        key: "swift_feather",
        name: "Swift Feather",
        spell: 2479,
        bonus: Bonus::pct(Stat::Speed, 0.06),
        rarity: CardRarity::Common,
        effect: None,
    },
    RunItemDef {
        key: "lodestone",
        name: "Lodestone",
        spell: 1906,
        bonus: Bonus::pct(Stat::Magnet, 0.15),
        rarity: CardRarity::Common,
        effect: None,
    },
    RunItemDef {
        key: "tome_of_lore",
        name: "Tome of Lore",
        spell: 1459,
        bonus: Bonus::flat(Stat::Growth, 0.08),
        rarity: CardRarity::Common,
        effect: None,
    },
    RunItemDef {
        key: "giants_belt",
        name: "Giant's Belt",
        spell: 2458,
        bonus: Bonus::pct(Stat::MaxHp, 0.08),
        rarity: CardRarity::Uncommon,
        effect: None,
    },
    RunItemDef {
        key: "arcane_prism",
        name: "Arcane Prism",
        spell: 1449,
        bonus: Bonus::flat(Stat::Area, 0.06),
        rarity: CardRarity::Rare,
        effect: None,
    },
    RunItemDef {
        key: "lucky_coin",
        name: "Lucky Coin",
        spell: 1784,
        bonus: Bonus::pct(Stat::Gold, 0.15),
        rarity: CardRarity::Uncommon,
        effect: None,
    },
    RunItemDef {
        key: "stoneskin",
        name: "Stoneskin Charm",
        spell: 20594,
        bonus: Bonus::flat(Stat::DamageReduction, 0.02),
        rarity: CardRarity::Rare,
        effect: None,
    },
    RunItemDef {
        key: "thornmail",
        name: "Thornmail Bracers",
        spell: 467,
        bonus: Bonus::flat(Stat::Armor, 1.0),
        rarity: CardRarity::Uncommon,
        effect: Some(ItemEffect::Thorns(6.0)),
    },
    RunItemDef {
        key: "vampiric_fang",
        name: "Vampiric Fang",
        spell: 15286,
        bonus: Bonus::flat(Stat::Might, 0.02),
        rarity: CardRarity::Rare,
        effect: Some(ItemEffect::Lifesteal(0.015)),
    },
    RunItemDef {
        key: "soulfeast",
        name: "Soulfeast Shard",
        spell: 1120,
        bonus: Bonus::flat(Stat::Regen, 0.3),
        rarity: CardRarity::Rare,
        effect: Some(ItemEffect::KillHeal(1.5)),
    },
    RunItemDef {
        key: "storm_charm",
        name: "Storm Charm",
        spell: 421,
        bonus: Bonus::flat(Stat::Haste, 0.01),
        rarity: CardRarity::Rare,
        effect: Some(ItemEffect::Chain {
            chance: 0.06,
            damage: 10.0,
            jumps: 2,
            spell: 421,
        }),
    },
    RunItemDef {
        key: "volatile_ember",
        name: "Volatile Ember",
        spell: 11113,
        bonus: Bonus::flat(Stat::Area, 0.02),
        rarity: CardRarity::Epic,
        effect: Some(ItemEffect::Burst {
            chance: 0.15,
            damage: 18.0,
            radius: 5.0,
            spell: 11113,
        }),
    },
    RunItemDef {
        key: "thunderfury_shard",
        name: "Shard of the Windlord",
        spell: 930,
        bonus: Bonus::flat(Stat::Might, 0.05),
        rarity: CardRarity::Legendary,
        effect: Some(ItemEffect::Chain {
            chance: 0.12,
            damage: 22.0,
            jumps: 4,
            spell: 930,
        }),
    },
    // M8: more ways to build a run.
    RunItemDef {
        key: "smoldering_brand",
        name: "Smoldering Brand",
        spell: 348,
        bonus: Bonus::flat(Stat::Might, 0.02),
        rarity: CardRarity::Uncommon,
        effect: Some(ItemEffect::Ignite {
            chance: 0.08,
            dps: 5.0,
            secs: 4.0,
            spell: 348,
        }),
    },
    RunItemDef {
        key: "frostbite_locket",
        name: "Frostbite Locket",
        spell: 116,
        bonus: Bonus::flat(Stat::Area, 0.02),
        rarity: CardRarity::Uncommon,
        effect: Some(ItemEffect::Chill {
            chance: 0.12,
            slow: 0.5,
            secs: 2.0,
        }),
    },
    RunItemDef {
        key: "headsmans_axe",
        name: "Headsman's Axe",
        spell: 5308,
        bonus: Bonus::flat(Stat::Crit, 0.01),
        rarity: CardRarity::Rare,
        effect: Some(ItemEffect::Execute(0.06)),
    },
    RunItemDef {
        key: "evasion_charm",
        name: "Shadowstep Charm",
        spell: 5277,
        bonus: Bonus::pct(Stat::Speed, 0.03),
        rarity: CardRarity::Rare,
        effect: Some(ItemEffect::Dodge(0.04)),
    },
    RunItemDef {
        key: "barrier_sigil",
        name: "Sigil of Warding",
        spell: 17,
        bonus: Bonus::flat(Stat::Armor, 1.0),
        rarity: CardRarity::Uncommon,
        effect: Some(ItemEffect::KillShield {
            amount: 1.5,
            max: 25.0,
        }),
    },
    RunItemDef {
        key: "holy_relic",
        name: "Reliquary of Light",
        spell: 15237,
        bonus: Bonus::flat(Stat::Regen, 0.3),
        rarity: CardRarity::Epic,
        effect: Some(ItemEffect::Nova {
            every: 5.0,
            damage: 16.0,
            radius: 6.0,
            spell: 15237,
        }),
    },
    RunItemDef {
        key: "goblin_purse",
        name: "Goblin's Purse",
        spell: 1804,
        bonus: Bonus::pct(Stat::Gold, 0.05),
        rarity: CardRarity::Common,
        effect: Some(ItemEffect::Bounty {
            chance: 0.08,
            gold: 1.0,
        }),
    },
    RunItemDef {
        key: "ashbringer_fragment",
        name: "Ashbringer Fragment",
        spell: 26573,
        bonus: Bonus::flat(Stat::Might, 0.05),
        rarity: CardRarity::Legendary,
        effect: Some(ItemEffect::Ignite {
            chance: 0.25,
            dps: 14.0,
            secs: 5.0,
            spell: 26573,
        }),
    },
    RunItemDef {
        key: "frost_wyrm_scale",
        name: "Frost Wyrm Scale",
        spell: 10230,
        bonus: Bonus::flat(Stat::Armor, 3.0),
        rarity: CardRarity::Legendary,
        effect: Some(ItemEffect::Nova {
            every: 3.0,
            damage: 26.0,
            radius: 8.0,
            spell: 10230,
        }),
    },
];

/// A run item by key.
pub(crate) fn run_item(key: &str) -> Option<&'static RunItemDef> {
    RUN_ITEMS.iter().find(|i| i.key == key)
}

/// A run item drawn: its rarity from `rarity_roll` by [`CardRarity::item_odds`] (among the
/// rarities some item has), then one of that rarity from `item_roll`; both uniform in `[0, 1)`.
pub(crate) fn draw_run_item(rarity_roll: f32, item_roll: f32) -> &'static RunItemDef {
    let held: Vec<CardRarity> = CardRarity::ALL
        .into_iter()
        .filter(|r| RUN_ITEMS.iter().any(|i| i.rarity == *r))
        .collect();
    let total: f32 = held.iter().map(|r| r.item_odds()).sum();
    let mut at = rarity_roll.clamp(0.0, 0.999_999) * total;
    let mut rarity = held[held.len() - 1];
    for r in &held {
        if at < r.item_odds() {
            rarity = *r;
            break;
        }
        at -= r.item_odds();
    }
    let of: Vec<&RunItemDef> = RUN_ITEMS.iter().filter(|i| i.rarity == rarity).collect();
    of[((item_roll.clamp(0.0, 0.999_999)) * of.len() as f32) as usize]
}

/// A run item drawn among `held`, each as likely as its rarity ([`CardRarity::item_odds`]);
/// `roll` uniform in `[0, 1)`. `None` when `held` is empty.
pub(crate) fn draw_held_run_item(
    held: &[&'static RunItemDef],
    roll: f32,
) -> Option<&'static RunItemDef> {
    let total: f32 = held.iter().map(|i| i.rarity.item_odds()).sum();
    let mut at = roll.clamp(0.0, 0.999_999) * total;
    for i in held {
        if at < i.rarity.item_odds() {
            return Some(i);
        }
        at -= i.rarity.item_odds();
    }
    held.last().copied()
}

/// What a chest, or an elite's drop, gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reward {
    /// A random run item.
    RunItem,
    /// A rank on one of the hero's spells.
    SpellRank,
    /// A roll for one of the zone's pieces of gear, `percent` likely; gold otherwise.
    Gear { percent: u8 },
    /// Gold.
    Gold(u32),
}

/// What a chest gives, from `roll` (uniform in `[0, 1)`): a run item more often than not, else a
/// spell's rank, else a fair roll for gear.
pub(crate) fn chest_reward(roll: f32) -> Reward {
    if roll < 0.55 {
        Reward::RunItem
    } else if roll < 0.85 {
        Reward::SpellRank
    } else {
        Reward::Gear { percent: 60 }
    }
}

/// The field's elites: where each stands, what it guards, how long it takes to come back.
pub(crate) struct EliteKind {
    pub title: &'static str,
    pub reward: Reward,
    pub respawn: f32,
    /// The `Spell.dbc` row whose icon shows the reward over the elite (the icon only).
    pub icon: u32,
    /// Its marks' colour, on the ground, the minimap and its label (sRGB, 0..1).
    pub color: [f32; 3],
}

/// The `Spell.dbc` row whose icon shows over a chest: Pick Pocket's purse.
pub(crate) const CHEST_ICON: u32 = 921;
/// A chest's marks' colour.
pub(crate) const CHEST_COLOR: [f32; 3] = [1.0, 0.82, 0.2];
/// Yards from the hero within which the field's labels show over the world.
pub(crate) const MARK_RANGE: f32 = 90.0;

pub(crate) const ELITES: [EliteKind; 4] = [
    EliteKind {
        title: "Armory Guardian",
        reward: Reward::Gear { percent: 35 },
        respawn: 90.0,
        icon: 13262,
        color: [0.64, 0.21, 0.93],
    },
    EliteKind {
        title: "Arcane Sentinel",
        reward: Reward::SpellRank,
        respawn: 75.0,
        icon: 1459,
        color: [0.25, 0.6, 1.0],
    },
    EliteKind {
        title: "Relic Keeper",
        reward: Reward::RunItem,
        respawn: 75.0,
        icon: 2481,
        color: [0.12, 1.0, 0.0],
    },
    EliteKind {
        title: "Hoard Warden",
        reward: Reward::Gold(40),
        respawn: 60.0,
        icon: 921,
        color: [1.0, 0.5, 0.0],
    },
];

/// An elite of a zone: the strongest of its horde, grown and toughened, named for what it guards.
pub(crate) fn elite(zone: usize, kind: usize) -> &'static EnemyDef {
    static ELITE_DEFS: std::sync::LazyLock<Vec<Vec<EnemyDef>>> = std::sync::LazyLock::new(|| {
        (0..MAPS.len())
            .map(|z| {
                let r = roster(z);
                let top = r.enemies.last().expect("a zone has a horde");
                ELITES
                    .iter()
                    .enumerate()
                    .map(|(k, e)| {
                        // Each elite wears another of the zone's stronger creatures.
                        let n = r.enemies.len();
                        let look = &r.enemies[n.saturating_sub(1 + k).max(n / 2)];
                        EnemyDef {
                            name: Box::leak(format!("{} {}", look.name, e.title).into_boxed_str()),
                            display: look.display,
                            hp: (top.hp * 8.0).round(),
                            speed: look.speed * 0.9,
                            damage: (top.damage * 1.5).round(),
                            scale: look.scale * 1.6,
                            xp: top.xp * 6,
                            level: look.level + 2,
                            big: look.big,
                        }
                    })
                    .collect()
            })
            .collect()
    });
    &ELITE_DEFS[zone.min(MAPS.len() - 1)][kind.min(ELITES.len() - 1)]
}

/// What a shrine of the field does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ShrineKind {
    /// Stand in it until it is charged: the hero in it chooses one blessing of three.
    Charge,
    /// Wake it: elites come for the heroes, and a free chest stands where they fell.
    Challenge,
    /// More gold for every hero for the rest of the run, and fiercer foes.
    Greed,
    /// More experience for every hero for the rest of the run, and more foes, tougher.
    Curse,
}

/// A shrine of the field: its kind, how it reads, how long a hero stands in it to wake it, and
/// how many of it the field holds.
pub(crate) struct ShrineDef {
    pub kind: ShrineKind,
    pub title: &'static str,
    /// What it does, as its label over the world reads.
    pub text: &'static str,
    /// The `Spell.dbc` row for its icon (the icon only).
    pub spell: u32,
    /// Its marks' colour (sRGB, 0..1).
    pub color: [f32; 3],
    /// Seconds a hero stands in it to wake it.
    pub hold: f32,
    pub count: usize,
}

pub(crate) const SHRINES: [ShrineDef; 4] = [
    ShrineDef {
        kind: ShrineKind::Charge,
        title: "Shrine of Power",
        text: "Stand in it to charge it, then choose a blessing",
        spell: 10060,
        color: [1.0, 0.85, 0.3],
        hold: 5.0,
        count: 3,
    },
    ShrineDef {
        kind: ShrineKind::Challenge,
        title: "Shrine of Challenge",
        text: "Calls elites: beat them for a free chest",
        spell: 6673,
        color: [0.95, 0.25, 0.15],
        hold: 1.5,
        count: 1,
    },
    ShrineDef {
        kind: ShrineKind::Greed,
        title: "Shrine of Greed",
        text: "More gold for everyone, fiercer foes",
        spell: 921,
        color: [1.0, 0.65, 0.1],
        hold: 1.5,
        count: 1,
    },
    ShrineDef {
        kind: ShrineKind::Curse,
        title: "Shrine of Curses",
        text: "More experience for everyone, more foes",
        spell: 702,
        color: [0.6, 0.25, 0.85],
        hold: 1.5,
        count: 1,
    },
];

/// A shrine's kind, by its [`SHRINES`] index.
pub(crate) fn shrine(kind: usize) -> &'static ShrineDef {
    &SHRINES[kind.min(SHRINES.len() - 1)]
}

/// How far from a shrine's middle a hero stands in it.
pub(crate) const SHRINE_RADIUS: f32 = 4.0;
/// How fast a shrine left alone loses its charge, against how fast it gains it.
pub(crate) const SHRINE_DECAY: f32 = 0.5;
/// The elites a Shrine of Challenge calls.
pub(crate) const CHALLENGE_ELITES: usize = 3;
/// What each Shrine of Greed gives every hero, and does to the foes from then on.
pub(crate) const GREED_GOLD: Bonus = Bonus::pct(Stat::Gold, 0.25);
pub(crate) const GREED_HEALTH: f32 = 1.1;
pub(crate) const GREED_DAMAGE: f32 = 1.15;
/// What each Shrine of Curses gives every hero, and does to the foes from then on.
pub(crate) const CURSE_GROWTH: Bonus = Bonus::flat(Stat::Growth, 0.12);
pub(crate) const CURSE_HEALTH: f32 = 1.1;
pub(crate) const CURSE_DENSITY: f32 = 1.25;

/// A blessing a Shrine of Power offers: one stat, at a drawn rarity's worth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Boon {
    pub bonus: Bonus,
    pub rarity: CardRarity,
}

/// The blessings' stats and a Common one's worth; a rarer one is worth [`CardRarity::weight`]
/// times as much.
pub(crate) const BOONS: [Bonus; 12] = [
    Bonus::flat(Stat::Might, 0.08),
    Bonus::pct(Stat::MaxHp, 0.08),
    Bonus::pct(Stat::Speed, 0.05),
    Bonus::flat(Stat::Regen, 0.8),
    Bonus::flat(Stat::Haste, 0.05),
    Bonus::flat(Stat::Area, 0.06),
    Bonus::pct(Stat::Magnet, 0.2),
    Bonus::flat(Stat::Growth, 0.08),
    Bonus::flat(Stat::Armor, 2.0),
    Bonus::flat(Stat::Crit, 0.03),
    Bonus::flat(Stat::DamageReduction, 0.02),
    Bonus::pct(Stat::Gold, 0.12),
];

/// The blessing of `BOONS[stat]` at `rarity`.
pub(crate) fn boon(stat: usize, rarity: CardRarity) -> Boon {
    let base = BOONS[stat.min(BOONS.len() - 1)];
    let k = rarity.weight();
    let amount = match base.amount {
        Amount::Flat(v) => Amount::Flat(v * k),
        Amount::Percent(v) => Amount::Percent(v * k),
    };
    Boon {
        bonus: Bonus {
            stat: base.stat,
            amount,
        },
        rarity,
    }
}

/// Rerolls of the level-up cards a run starts with (R at the cards).
pub(crate) const REROLLS_PER_RUN: u8 = 2;
/// Banishes a run starts with: a card taken out of the hero's deals for the rest of the run.
pub(crate) const BANISHES_PER_RUN: u8 = 3;
/// What skipping a level-up pays, before the gold stat. A skip is always there to take.
pub(crate) const SKIP_GOLD: u32 = 10;
/// Every this many levels, the hero earns one more banish or reroll, in turn.
pub(crate) const CARD_TOKEN_LEVELS: u32 = 5;

// --- How a hit feels (`feel.rs`). ---

/// The shove every hit gives a foe, in yd/s, backward from the hero it chases.
pub(crate) const KNOCK_MIN: f32 = 1.5;
/// What more a heavy hit adds, at its fullest for a hit of [`KNOCK_SHARE`] of the foe's health.
pub(crate) const KNOCK_SPEED: f32 = 7.0;
pub(crate) const KNOCK_SHARE: f32 = 0.2;
/// The most a foe is shoved at once, however many hits land.
pub(crate) const KNOCK_MAX: f32 = 10.0;
/// How fast a shove dies, per second: a foe slides `speed / KNOCK_DECAY` yards.
pub(crate) const KNOCK_DECAY: f32 = 9.0;
/// Bosses and elites hold their ground better.
pub(crate) const KNOCK_BOSS: f32 = 0.1;
pub(crate) const KNOCK_ELITE: f32 = 0.35;

/// The shove a hit of `damage` gives a foe of `max_hp`.
pub(crate) fn knock_impulse(damage: f32, max_hp: f32, boss: bool, elite: bool) -> f32 {
    if damage <= 0.0 {
        return 0.0;
    }
    let heavy = (damage / (max_hp.max(1.0) * KNOCK_SHARE)).min(1.0);
    let hold = if boss {
        KNOCK_BOSS
    } else if elite {
        KNOCK_ELITE
    } else {
        1.0
    };
    (KNOCK_MIN + KNOCK_SPEED * heavy) * hold
}

/// Seconds a struck model stays brightened.
pub(crate) const FLASH_SECS: f32 = 0.12;
/// The screen's shake: trauma (0..1) a blow to the hero adds per share of its health taken, and
/// what a critical strike adds, crits alone shaking it at most [`SHAKE_CRIT_MAX`].
pub(crate) const SHAKE_PER_HURT: f32 = 2.5;
pub(crate) const SHAKE_CRIT: f32 = 0.04;
pub(crate) const SHAKE_CRIT_MAX: f32 = 0.2;
/// A boss's coming shakes it this hard.
pub(crate) const SHAKE_BOSS: f32 = 0.6;
/// Trauma lost per second, and the most it moves the camera (yards) and turns it (radians).
pub(crate) const SHAKE_DECAY: f32 = 1.6;
pub(crate) const SHAKE_OFFSET: f32 = 0.3;
pub(crate) const SHAKE_TURN: f32 = 0.02;
/// The most of each a hero can hold.
pub(crate) const MAX_CARD_TOKENS: u8 = 9;
/// Cards on offer at a level-up.
pub(crate) const CARDS_DEALT: usize = 4;

/// What one level-up card does to a spell: a single stat of it, the rarer the more, and an Epic
/// or Legendary card two or three of them. A spell's ranks have no cap: each card adds its perks
/// to what the spell already has ([`Perks`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Perk {
    /// More damage, a share of the spell's own.
    Damage(f32),
    /// A shorter cooldown: this share off what it is.
    Cooldown(f32),
    /// More enemies struck.
    Targets(u8),
    /// A wider area, a share of the spell's own.
    Area(f32),
    /// A longer effect, a share of the spell's own.
    Duration(f32),
}

/// The perk kinds, in their wire order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PerkKind {
    Damage,
    Cooldown,
    Targets,
    Area,
    Duration,
}

impl PerkKind {
    pub const ALL: [PerkKind; 5] = [
        PerkKind::Damage,
        PerkKind::Cooldown,
        PerkKind::Targets,
        PerkKind::Area,
        PerkKind::Duration,
    ];

    /// Whether a spell has this stat to grow: damage for one that deals it, targets for one that
    /// picks a number of them, area and duration for one that has them; every one a cooldown.
    pub fn fits(self, def: &AbilityDef) -> bool {
        match self {
            PerkKind::Damage => def.damage > 0.0 || def.dot > 0.0,
            PerkKind::Cooldown => def.cooldown > 0.0,
            PerkKind::Targets => matches!(
                def.kind,
                Kind::Bolt | Kind::Strike | Kind::Melee | Kind::Dot | Kind::Chain | Kind::Orbit
            ),
            PerkKind::Area => def.radius > 0.0,
            PerkKind::Duration => def.duration > 0.0,
        }
    }

    /// This stat's perk at `rarity`.
    pub fn at(self, rarity: CardRarity) -> Perk {
        let r = rarity as usize;
        match self {
            PerkKind::Damage => Perk::Damage([0.12, 0.18, 0.25, 0.35, 0.5][r]),
            PerkKind::Cooldown => Perk::Cooldown([0.06, 0.08, 0.1, 0.13, 0.17][r]),
            PerkKind::Targets => Perk::Targets(if rarity == CardRarity::Legendary {
                2
            } else {
                1
            }),
            PerkKind::Area => Perk::Area([0.08, 0.12, 0.16, 0.22, 0.3][r]),
            PerkKind::Duration => Perk::Duration([0.1, 0.15, 0.2, 0.28, 0.4][r]),
        }
    }
}

impl Perk {
    pub fn kind(self) -> PerkKind {
        match self {
            Perk::Damage(_) => PerkKind::Damage,
            Perk::Cooldown(_) => PerkKind::Cooldown,
            Perk::Targets(_) => PerkKind::Targets,
            Perk::Area(_) => PerkKind::Area,
            Perk::Duration(_) => PerkKind::Duration,
        }
    }

    /// `(kind, value)` as the wire carries it.
    pub fn to_wire(self) -> (u8, f32) {
        let k = self.kind() as u8;
        let v = match self {
            Perk::Damage(v) | Perk::Cooldown(v) | Perk::Area(v) | Perk::Duration(v) => v,
            Perk::Targets(n) => f32::from(n),
        };
        (k, v)
    }

    pub fn from_wire((k, v): (u8, f32)) -> Option<Perk> {
        if !v.is_finite() {
            return None;
        }
        Some(match PerkKind::ALL.get(usize::from(k))? {
            PerkKind::Damage => Perk::Damage(v),
            PerkKind::Cooldown => Perk::Cooldown(v),
            PerkKind::Targets => Perk::Targets(v.clamp(0.0, 255.0) as u8),
            PerkKind::Area => Perk::Area(v),
            PerkKind::Duration => Perk::Duration(v),
        })
    }
}

/// How many stats a card of `rarity` grows: one, two for an Epic, three for a Legendary.
pub(crate) fn perks_for(rarity: CardRarity) -> usize {
    match rarity {
        CardRarity::Epic => 2,
        CardRarity::Legendary => 3,
        _ => 1,
    }
}

/// What a spell's cards have given it, summed: the share of damage added, what is left of its
/// cooldown, the targets added, the shares of area and duration added.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Perks {
    pub damage: f32,
    pub cooldown: f32,
    pub targets: u8,
    pub area: f32,
    pub duration: f32,
}

impl Default for Perks {
    fn default() -> Self {
        Self {
            damage: 0.0,
            cooldown: 1.0,
            targets: 0,
            area: 0.0,
            duration: 0.0,
        }
    }
}

/// The least of its cooldown a spell keeps, however many cards cut it.
pub(crate) const MIN_COOLDOWN_SHARE: f32 = 0.25;
/// The most cards add to a spell's area, duration and targets: its damage grows without end, but
/// a ground spell past these covers the field and stays forever (the simulator's runaway).
pub(crate) const MAX_PERK_AREA: f32 = 1.5;
pub(crate) const MAX_PERK_DURATION: f32 = 2.0;
pub(crate) const MAX_PERK_TARGETS: u8 = 10;

impl Perks {
    /// These with `perk` added: shares add up, a cooldown is cut by its share of what is left.
    pub fn with(mut self, perk: Perk) -> Self {
        match perk {
            Perk::Damage(v) => self.damage += v,
            Perk::Cooldown(v) => {
                self.cooldown = (self.cooldown * (1.0 - v)).max(MIN_COOLDOWN_SHARE);
            }
            Perk::Targets(n) => {
                self.targets = self.targets.saturating_add(n).min(MAX_PERK_TARGETS)
            }
            Perk::Area(v) => self.area = (self.area + v).min(MAX_PERK_AREA),
            Perk::Duration(v) => self.duration = (self.duration + v).min(MAX_PERK_DURATION),
        }
        self
    }
}

/// A rarity of the run's draws: a level-up card's, the rarer the more its rank is worth, and a
/// run item's, the rarer the less often drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub(crate) enum CardRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl CardRarity {
    pub const ALL: [CardRarity; 5] = [
        CardRarity::Common,
        CardRarity::Uncommon,
        CardRarity::Rare,
        CardRarity::Epic,
        CardRarity::Legendary,
    ];

    /// How likely each rarity is dealt, in parts of 100.
    pub const fn odds(self) -> f32 {
        match self {
            CardRarity::Common => 52.0,
            CardRarity::Uncommon => 28.0,
            CardRarity::Rare => 13.0,
            CardRarity::Epic => 5.5,
            CardRarity::Legendary => 1.5,
        }
    }

    /// How likely a run item of this rarity is drawn, in parts of 100.
    pub const fn item_odds(self) -> f32 {
        match self {
            CardRarity::Common => 45.0,
            CardRarity::Uncommon => 30.0,
            CardRarity::Rare => 16.0,
            CardRarity::Epic => 7.0,
            CardRarity::Legendary => 2.0,
        }
    }

    /// The ranks' worth one card of this rarity adds: a spell's damage and a passive's bonus
    /// grow by it, where a rank's count, cooldown and reach grow by one rank as always.
    pub const fn weight(self) -> f32 {
        match self {
            CardRarity::Common => 1.0,
            CardRarity::Uncommon => 1.25,
            CardRarity::Rare => 1.5,
            CardRarity::Epic => 2.0,
            CardRarity::Legendary => 3.0,
        }
    }

    /// The rarity `roll` (uniform in `[0, 1)`) draws.
    pub fn draw(roll: f32) -> CardRarity {
        let total: f32 = CardRarity::ALL.iter().map(|r| r.odds()).sum();
        let mut at = roll.clamp(0.0, 0.999_999) * total;
        for r in CardRarity::ALL {
            if at < r.odds() {
                return r;
            }
            at -= r.odds();
        }
        CardRarity::Legendary
    }

    pub fn to_wire(self) -> u8 {
        self as u8
    }

    pub fn from_wire(n: u8) -> Option<CardRarity> {
        CardRarity::ALL.get(usize::from(n)).copied()
    }
}

/// A solo run's difficulty, picked under Run Options: each opens a zone at a time, once the one
/// below it has been won there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Difficulty {
    pub name: &'static str,
    /// Multipliers on enemy health and damage.
    pub health: f32,
    pub damage: f32,
    /// The multiplier on the run's gold.
    pub reward: f32,
}

pub(crate) const DIFFICULTIES: [Difficulty; 3] = [
    Difficulty {
        name: "Normal",
        health: 1.0,
        damage: 1.0,
        reward: 1.0,
    },
    Difficulty {
        name: "Heroic",
        health: 1.7,
        damage: 1.5,
        reward: 1.75,
    },
    Difficulty {
        name: "Mythic",
        health: 2.6,
        damage: 2.1,
        reward: 2.75,
    },
];

impl Rules {
    /// The solo rules at a difficulty ([`DIFFICULTIES`] index, held to the table).
    pub fn solo_at(difficulty: usize) -> Self {
        let d = DIFFICULTIES[difficulty.min(DIFFICULTIES.len() - 1)];
        Self {
            health: d.health,
            damage: d.damage,
            ..Self::SOLO
        }
    }
}

/// The gold a run earns: a coin for every eight kills, 25 for each boss reached, 100 for the win,
/// all times the hero's gold multiplier ([`Stat::Gold`]).
pub(crate) fn run_gold(kills: u32, bosses: u32, won: bool, gold: f32) -> u32 {
    let base = kills / 8 + bosses * 25 + if won { 100 } else { 0 };
    (base as f32 * gold.max(0.0)).round() as u32
}

/// What an achievement asks for, read off the profile.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Goal {
    /// Runs finished, won or lost.
    Runs(u32),
    /// Runs won, anywhere.
    Wins(u32),
    /// Enemies slain over every run.
    Kills(u64),
    /// The campaign's zone ([`MAPS`] index) won.
    WinZone(usize),
    /// Any zone won at a difficulty ([`DIFFICULTIES`] index) or harder.
    WinAt(usize),
    /// Pieces in the collection.
    Collect(usize),
    /// Bosses in the Codex.
    BossesMet(usize),
    /// Talent points trained.
    Points(u32),
    /// A hero's level reached in one run.
    Level(u32),
}

/// An achievement of the mode: what it asks and the gold it pays once claimed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AchievementDef {
    /// The profile stores claims by it.
    pub key: String,
    pub name: String,
    pub text: String,
    pub goal: Goal,
    pub gold: u64,
}

impl AchievementDef {
    /// The reward points it earns once done, a tenth of its gold: spent on the Rewards tab's
    /// perks, races, tabards and companions.
    pub fn points(&self) -> u32 {
        (self.gold / 10) as u32
    }
}

/// A perk: an account-wide edge every hero of every class brings into a run, bought rank by
/// rank with reward points on the Rewards tab.
#[derive(Debug, PartialEq)]
pub(crate) struct PerkDef {
    /// The profile stores ranks by it.
    pub key: &'static str,
    pub name: &'static str,
    /// What one rank does; `{}` is the rank's amount.
    pub text: &'static str,
    /// Icon, a `Spell.dbc` row.
    pub spell: u32,
    pub max_rank: u8,
    /// Points per rank.
    pub cost: u32,
    pub effect: PerkEffect,
}

/// What a perk's rank does to a hero at the start of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PerkEffect {
    /// More rerolls or banishes in hand.
    Rerolls(u8),
    Banishes(u8),
    /// More gold a skipped hand pays.
    SkipGold(u32),
    /// The run's first level-up deals this many more cards.
    OpeningHand(u8),
    /// A bonus held all run, as the profile's other bonuses.
    Bonus(Bonus),
    /// Gold in hand when the run begins.
    StartGold(u32),
}

impl PerkDef {
    /// What `rank` ranks amount to, for the description (one rank's, at none).
    pub fn amount_at(&self, rank: u8) -> String {
        let k = rank.max(1);
        match self.effect {
            PerkEffect::Rerolls(n) | PerkEffect::Banishes(n) | PerkEffect::OpeningHand(n) => {
                (u32::from(n) * u32::from(k)).to_string()
            }
            PerkEffect::StartGold(n) | PerkEffect::SkipGold(n) => (n * u32::from(k)).to_string(),
            PerkEffect::Bonus(b) => match b.amount {
                Amount::Flat(v) | Amount::Percent(v) => {
                    format!("{}", (v * 100.0 * f32::from(k)).round())
                }
            },
        }
    }
}

/// The perks, in the order the Rewards tab lists them.
pub(crate) static PERKS: &[PerkDef] = &[
    PerkDef {
        key: "second_thoughts",
        name: "Second Thoughts",
        text: "Rerolls every run: +{}.",
        spell: 25771,
        max_rank: 3,
        cost: 10,
        effect: PerkEffect::Rerolls(1),
    },
    PerkDef {
        key: "banish",
        name: "Banish",
        text: "Banishes every run: +{}. A banished card is never dealt again.",
        spell: 710,
        max_rank: 3,
        cost: 10,
        effect: PerkEffect::Banishes(1),
    },
    PerkDef {
        key: "skip",
        name: "Skip",
        text: "A skipped hand pays {} more gold.",
        spell: 1953,
        max_rank: 2,
        cost: 10,
        effect: PerkEffect::SkipGold(5),
    },
    PerkDef {
        key: "opening_hand",
        name: "Opening Hand",
        text: "Cards on your first level-up of every run: +{}.",
        spell: 6197,
        max_rank: 1,
        cost: 25,
        effect: PerkEffect::OpeningHand(1),
    },
    PerkDef {
        key: "veterans_lessons",
        name: "Veteran's Lessons",
        text: "Experience: +{}%.",
        spell: 1459,
        max_rank: 3,
        cost: 15,
        effect: PerkEffect::Bonus(Bonus::pct(Stat::Growth, 0.05)),
    },
    PerkDef {
        key: "keen_eye",
        name: "Keen Eye",
        text: "Gold: +{}%.",
        spell: 1784,
        max_rank: 3,
        cost: 15,
        effect: PerkEffect::Bonus(Bonus::pct(Stat::Gold, 0.05)),
    },
    PerkDef {
        key: "war_chest",
        name: "War Chest",
        text: "Gold in hand when a run begins: {}.",
        spell: 23880,
        max_rank: 3,
        cost: 15,
        effect: PerkEffect::StartGold(20),
    },
];

/// A perk by its key.
pub(crate) fn perk(key: &str) -> Option<&'static PerkDef> {
    PERKS.iter().find(|p| p.key == key)
}

/// How many times a save may take back every point spent on perks.
pub(crate) const MAX_PERK_RESETS: u8 = 2;

/// What opening a race to every class costs, in reward points. An opened race stays open: a
/// perk reset leaves it.
pub(crate) const RACE_POINTS: u32 = 100;

/// The playable races, `ChrRaces.dbc` ids.
pub(crate) const RACES: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// The classes a race opened in the Rewards tab can be beyond vanilla's ([`race_classes`]).
pub(crate) fn race_opens(race: u8) -> impl Iterator<Item = &'static ClassDef> {
    CLASSES
        .iter()
        .filter(move |c| !race_classes(race).contains(&c.id))
}

/// Every achievement, in the order the tab lists them.
pub(crate) static ACHIEVEMENTS: std::sync::LazyLock<Vec<AchievementDef>> =
    std::sync::LazyLock::new(|| {
        let a = |key: &str, name: &str, text: &str, goal: Goal, gold: u64| AchievementDef {
            key: key.into(),
            name: name.into(),
            text: text.into(),
            goal,
            gold,
        };
        let mut out = vec![
            a(
                "first_steps",
                "First Steps",
                "Finish a run.",
                Goal::Runs(1),
                50,
            ),
            a(
                "survivor",
                "Survivor",
                "Finish 10 runs.",
                Goal::Runs(10),
                150,
            ),
            a("veteran", "Veteran", "Finish 50 runs.", Goal::Runs(50), 500),
            a("victor", "Victor", "Win a run.", Goal::Wins(1), 100),
            a("champion", "Champion", "Win 25 runs.", Goal::Wins(25), 750),
            a(
                "slayer",
                "Slayer",
                "Slay 1,000 enemies.",
                Goal::Kills(1_000),
                100,
            ),
            a(
                "exterminator",
                "Exterminator",
                "Slay 10,000 enemies.",
                Goal::Kills(10_000),
                400,
            ),
            a(
                "annihilator",
                "Annihilator",
                "Slay 50,000 enemies.",
                Goal::Kills(50_000),
                1_000,
            ),
            a(
                "seasoned",
                "Seasoned",
                "Reach level 30 in a run.",
                Goal::Level(30),
                150,
            ),
            a(
                "ascendant",
                "Ascendant",
                "Reach level 60 in a run.",
                Goal::Level(60),
                500,
            ),
            a(
                "collector",
                "Collector",
                "Collect 10 pieces.",
                Goal::Collect(10),
                150,
            ),
            a(
                "curator",
                "Curator",
                "Collect 30 pieces.",
                Goal::Collect(30),
                400,
            ),
            a(
                "completionist",
                "Completionist",
                "Collect all 60 pieces.",
                Goal::Collect(60),
                1_500,
            ),
            a(
                "monster_hunter",
                "Monster Hunter",
                "Meet 30 bosses.",
                Goal::BossesMet(30),
                300,
            ),
            a(
                "talented",
                "Talented",
                "Train 10 talent points.",
                Goal::Points(10),
                100,
            ),
            a(
                "master",
                "Master of the Arts",
                "Train all 51 talent points.",
                Goal::Points(51),
                1_000,
            ),
            a(
                "heroic",
                "Heroic",
                "Win a zone on Heroic.",
                Goal::WinAt(1),
                300,
            ),
            a(
                "mythic",
                "Mythic",
                "Win a zone on Mythic.",
                Goal::WinAt(2),
                800,
            ),
        ];
        for (z, m) in MAPS.iter().enumerate() {
            out.push(a(
                &format!("won {}", m.name),
                &format!("Hero of {}", m.name),
                &format!("Win {}: {}.", m.name, m.zone),
                Goal::WinZone(z),
                100 + 50 * z as u64,
            ));
        }
        out
    });

/// A bonus as a lobby loadout carries it: `(stat index, 0 flat or 1 percent, ten-thousandths)`.
pub(crate) fn bonus_to_wire(b: &Bonus) -> (u8, u8, i32) {
    let stat = Stat::ALL.iter().position(|s| *s == b.stat).unwrap_or(0) as u8;
    let (kind, v) = match b.amount {
        Amount::Flat(v) => (0, v),
        Amount::Percent(v) => (1, v),
    };
    (stat, kind, (v * 10_000.0).round() as i32)
}

/// The most a carried bonus may be worth, so a loadout cannot make a god of its hero.
const WIRE_BONUS_CAP: f32 = 1_000.0;

/// A carried bonus back, `None` for a stat the tables do not have; the value is held to
/// [`WIRE_BONUS_CAP`].
pub(crate) fn bonus_from_wire((stat, kind, v): (u8, u8, i32)) -> Option<Bonus> {
    let stat = *Stat::ALL.get(usize::from(stat))?;
    let v = (v as f32 / 10_000.0).clamp(-WIRE_BONUS_CAP, WIRE_BONUS_CAP);
    Some(if kind == 0 {
        Bonus::flat(stat, v)
    } else {
        Bonus::pct(stat, v)
    })
}

/// The share of an item's first bonus its place in the collection gives for good, worn or not.
pub(crate) const COLLECTION_SHARE: f32 = 0.25;

/// What having found an item gives every run from then on: a quarter of its first bonus.
pub(crate) fn collection_bonus(item: &ItemDef) -> Bonus {
    let b = item.bonuses[0];
    Bonus {
        stat: b.stat,
        amount: match b.amount {
            Amount::Flat(v) => Amount::Flat(v * COLLECTION_SHARE),
            Amount::Percent(v) => Amount::Percent(v * COLLECTION_SHARE),
        },
    }
}

/// Bonuses added up by stat and kind, in [`Stat::ALL`]'s order, flat before percent: the
/// collection's line on the main screen.
pub(crate) fn sum_bonuses(bonuses: &[Bonus]) -> Vec<Bonus> {
    let mut out = Vec::new();
    for stat in Stat::ALL {
        let (mut flat, mut pct) = (0.0f32, 0.0f32);
        for b in bonuses.iter().filter(|b| b.stat == stat) {
            match b.amount {
                Amount::Flat(v) => flat += v,
                Amount::Percent(v) => pct += v,
            }
        }
        if flat != 0.0 {
            out.push(Bonus::flat(stat, flat));
        }
        if pct != 0.0 {
            out.push(Bonus::pct(stat, pct));
        }
    }
    out
}

/// An item by its name.
pub(crate) fn item(name: &str) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|i| i.name == name)
}

/// The item a zone's `boss`th boss drops.
pub(crate) fn boss_item(zone: usize, boss: usize) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|i| i.zone == zone && i.boss == boss)
}

/// A zone's drop table: its five items, the first boss's first.
pub(crate) fn zone_items(zone: usize) -> Vec<&'static ItemDef> {
    ITEMS.iter().filter(|i| i.zone == zone).collect()
}

/// The odds one horde kill in a zone drops `item`: its share of [`HORDE_ITEM_ODDS`], the zone's
/// rarer half half as likely, as [`horde_item`] rolls it.
pub(crate) fn horde_chance(item: &ItemDef) -> f32 {
    let weight = |i: &ItemDef| if i.rarity <= Rarity::Rare { 2.0 } else { 1.0 };
    let table = zone_items(item.zone);
    let total: f32 = table.iter().map(|i| weight(i)).sum();
    weight(item) / total / HORDE_ITEM_ODDS as f32
}

/// The horde a run met in `zone` by `elapsed` under `rules`: every tier the waves have reached.
pub(crate) fn met_horde(zone: usize, elapsed: f32, rules: &Rules) -> Vec<&'static str> {
    let r = roster(zone);
    let top = rules.wave_pool(elapsed, r.enemies.len())[2].min(r.enemies.len() - 1);
    let mut out: Vec<&'static str> = Vec::new();
    for e in &r.enemies[..=top] {
        if !out.contains(&e.name) {
            out.push(e.name);
        }
    }
    out
}

/// Every creature the campaign holds, horde and bosses, each name once: the Codex's whole.
pub(crate) fn all_creatures() -> (Vec<&'static str>, Vec<&'static str>) {
    let (mut horde, mut bosses): (Vec<&'static str>, Vec<&'static str>) = (Vec::new(), Vec::new());
    for z in 0..MAPS.len() {
        let r = roster(z);
        for e in &r.enemies {
            if !horde.contains(&e.name) {
                horde.push(e.name);
            }
        }
        for b in &r.bosses {
            if !bosses.contains(&b.name) {
                bosses.push(b.name);
            }
        }
    }
    (horde, bosses)
}

/// The item a horde kill drops from `roll` (uniform in `[0, 1)`), if any: one in
/// [`HORDE_ITEM_ODDS`], the zone's commoner items likelier.
pub(crate) fn horde_item(zone: usize, roll: f32) -> Option<&'static ItemDef> {
    let odds = 1.0 / HORDE_ITEM_ODDS as f32;
    if roll >= odds {
        return None;
    }
    let table = zone_items(zone);
    // Rescaled into the table, the rarer half of it half as likely.
    let weights: Vec<f32> = table
        .iter()
        .map(|i| if i.rarity <= Rarity::Rare { 2.0 } else { 1.0 })
        .collect();
    let total: f32 = weights.iter().sum();
    let mut at = roll / odds * total;
    for (i, w) in table.iter().zip(&weights) {
        if at < *w {
            return Some(i);
        }
        at -= w;
    }
    table.last().copied()
}

/// A battleground's horde and bosses with their numbers.
pub(crate) struct Roster {
    pub enemies: Vec<EnemyDef>,
    pub bosses: Vec<EnemyDef>,
}

/// How much tougher the final boss is than the boss ladder's top rung.
pub(crate) const FINAL_BOSS_HEALTH: f32 = 1.5;
/// Every boss's health against the tuned ladder: a stage is a share of the old run's clock, and
/// a hero meets its final boss at that stage's strength, not the old run's end (the simulator
/// left it standing at 11% after half an hour).
pub(crate) const BOSS_HEALTH: f32 = 0.45;

static ROSTERS: std::sync::LazyLock<Vec<Roster>> = std::sync::LazyLock::new(|| {
    MAPS.iter()
        .map(|m| {
            let mut bosses = ladder(m.bosses, BOSSES, m.might);
            for b in &mut bosses {
                b.hp = (b.hp * BOSS_HEALTH).round();
            }
            if let Some(last) = bosses.last_mut() {
                last.hp = (last.hp * FINAL_BOSS_HEALTH).round();
            }
            Roster {
                enemies: ladder(m.foes, ENEMIES, m.might),
                bosses,
            }
        })
        .collect()
});

pub(crate) fn roster(map: usize) -> &'static Roster {
    &ROSTERS[map.min(ROSTERS.len() - 1)]
}

/// Number a roster off a tuned ladder: each creature takes the health, damage and experience at
/// its own place along it, from the ladder's running peak so no later tier comes out softer;
/// `might` multiplies the health and damage, for the zone's place in the campaign.
fn ladder(foes: &[Foe], tuned: &[EnemyDef], might: f32) -> Vec<EnemyDef> {
    let mut peak = (0.0f32, 0.0f32, 0.0f32);
    let rungs: Vec<(f32, f32, f32)> = tuned
        .iter()
        .map(|d| {
            peak = (
                peak.0.max(d.hp),
                peak.1.max(d.damage),
                peak.2.max(d.xp as f32),
            );
            peak
        })
        .collect();
    let top = (rungs.len() - 1) as f32;
    foes.iter()
        .enumerate()
        .map(|(i, f)| {
            let t = if foes.len() > 1 {
                i as f32 / (foes.len() - 1) as f32 * top
            } else {
                0.0
            };
            let (lo, hi) = (rungs[t.floor() as usize], rungs[t.ceil() as usize]);
            let k = t.fract();
            let mix = |a: f32, b: f32| (a + (b - a) * k).round();
            EnemyDef {
                name: f.name,
                display: f.display,
                hp: (mix(lo.0, hi.0) * might).round(),
                speed: f.speed,
                damage: (mix(lo.1, hi.1) * might).round(),
                scale: f.scale,
                xp: mix(lo.2, hi.2) as u32,
                level: f.level,
                big: f.big,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! The tables' invariants and the run's curves. Pure data: no app, no game files.

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ability_keys_are_unique() {
        let mut seen = HashSet::new();
        for a in ABILITIES {
            assert!(seen.insert(a.key), "ability {:?} is defined twice", a.key);
        }
    }

    #[test]
    fn passive_keys_are_unique() {
        let mut seen = HashSet::new();
        for p in PASSIVES {
            assert!(seen.insert(p.key), "passive {:?} is defined twice", p.key);
        }
    }

    /// `ability` panics on a missing key, at the menu or on the first card deal: catch it here.
    #[test]
    fn every_class_names_existing_abilities() {
        for c in CLASSES {
            assert!(
                ABILITIES.iter().any(|a| a.key == c.start),
                "{}: start ability {:?} is not in ABILITIES",
                c.name,
                c.start
            );
            for key in c.pool {
                assert!(
                    ABILITIES.iter().any(|a| a.key == *key),
                    "{}: pool ability {key:?} is not in ABILITIES",
                    c.name
                );
            }
        }
    }

    #[test]
    fn every_class_starts_with_an_ability_from_its_own_pool() {
        for c in CLASSES {
            assert!(
                c.pool.contains(&c.start),
                "{}: start {:?} is not in its pool",
                c.name,
                c.start
            );
        }
    }

    #[test]
    fn class_pools_have_no_duplicates() {
        for c in CLASSES {
            let set: HashSet<_> = c.pool.iter().collect();
            assert_eq!(
                set.len(),
                c.pool.len(),
                "{}: duplicate ability in pool",
                c.name
            );
        }
    }

    #[test]
    fn every_class_has_a_blurb() {
        for c in CLASSES {
            assert!(!c.blurb.is_empty(), "{}", c.name);
        }
    }

    #[test]
    fn every_class_has_looks_and_a_weapon() {
        for c in CLASSES {
            assert!(!c.looks.is_empty(), "{}: no looks", c.name);
            assert!(!c.weapons.is_empty(), "{}: no weapon", c.name);
            assert!(
                c.sheath == 1 || c.sheath == 2,
                "{}: sheath {}",
                c.name,
                c.sheath
            );
        }
    }

    #[test]
    fn abilities_have_sane_numbers() {
        for a in ABILITIES {
            assert!(a.cooldown > 0.0, "{}: cooldown {}", a.key, a.cooldown);
            assert!(a.count >= 1, "{}: count {}", a.key, a.count);
            for (what, v) in [
                ("damage", a.damage),
                ("dot", a.dot),
                ("range", a.range),
                ("radius", a.radius),
                ("duration", a.duration),
            ] {
                assert!(v.is_finite() && v >= 0.0, "{}: {what} {v}", a.key);
            }
            if a.kind == Kind::Totem {
                assert!(
                    a.totem.0 != 0 && a.totem.1 != 0,
                    "{}: totem without display or spell",
                    a.key
                );
            }
        }
    }

    #[test]
    fn run_items_draw_by_their_rarity_and_every_rarity_has_one() {
        for r in CardRarity::ALL {
            assert!(RUN_ITEMS.iter().any(|i| i.rarity == r), "{r:?}");
        }
        let mut keys: Vec<_> = RUN_ITEMS.iter().map(|i| i.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), RUN_ITEMS.len(), "keys are unique");
        let mut seen = [0u32; 5];
        let n = 20_000;
        for i in 0..n {
            let item = draw_run_item(i as f32 / n as f32, (i * 7 % 13) as f32 / 13.0);
            seen[item.rarity as usize] += 1;
        }
        for (r, got) in CardRarity::ALL.iter().zip(seen) {
            let want = r.item_odds() / 100.0 * n as f32;
            assert!(
                (got as f32 - want).abs() <= n as f32 * 0.002,
                "{r:?}: {got} for {want}"
            );
        }
        // Every item of a rarity can come up.
        for item in RUN_ITEMS {
            let of: Vec<_> = RUN_ITEMS
                .iter()
                .filter(|i| i.rarity == item.rarity)
                .collect();
            let k = of.iter().position(|i| i.key == item.key).unwrap();
            let roll = (k as f32 + 0.5) / of.len() as f32;
            let r = CardRarity::ALL
                .iter()
                .position(|r| *r == item.rarity)
                .unwrap();
            let below: f32 = CardRarity::ALL[..r].iter().map(|r| r.item_odds()).sum();
            let at = (below + item.rarity.item_odds() / 2.0) / 100.0;
            assert_eq!(draw_run_item(at, roll).key, item.key);
        }
    }

    #[test]
    fn item_effects_grow_with_stacks_and_their_chances_cap() {
        let fang = ItemEffect::Lifesteal(0.015);
        assert_eq!(fang.at(2), ItemEffect::Lifesteal(0.03));
        let charm = run_item("thunderfury_shard").unwrap().effect.unwrap();
        match charm.at(MAX_RUN_ITEM_STACKS) {
            ItemEffect::Chain { chance, damage, .. } => {
                assert_eq!(chance, MAX_PROC_CHANCE);
                assert!(damage > 22.0 * 8.0);
            }
            e => panic!("{e:?}"),
        }
        for i in RUN_ITEMS {
            if let Some(e) = i.effect {
                assert_ne!(e.at(1), e.at(3), "{} grows", i.key);
            }
        }
        // A nova strikes harder and a little sooner with each stack.
        let relic = run_item("holy_relic").unwrap().effect.unwrap();
        match (relic.at(1), relic.at(4)) {
            (
                ItemEffect::Nova {
                    every: a,
                    damage: d1,
                    ..
                },
                ItemEffect::Nova {
                    every: b,
                    damage: d4,
                    ..
                },
            ) => {
                assert!(b < a && b > a / 2.0);
                assert_eq!(d4, d1 * 4.0);
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn every_run_item_has_its_own_key_and_every_rarity_some() {
        let mut keys: Vec<&str> = RUN_ITEMS.iter().map(|i| i.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), RUN_ITEMS.len());
        for r in CardRarity::ALL {
            assert!(RUN_ITEMS.iter().any(|i| i.rarity == r), "{r:?}");
        }
        assert!(RUN_ITEMS.len() >= 27);
    }

    #[test]
    fn the_merchant_asks_more_for_the_rarer_and_more_each_time() {
        for w in CardRarity::ALL.windows(2) {
            assert!(shop_price(w[0], 0) < shop_price(w[1], 0));
        }
        assert_eq!(shop_price(CardRarity::Common, 0), 30);
        assert!(shop_price(CardRarity::Rare, 3) > shop_price(CardRarity::Rare, 2));
        for b in 0..80 {
            assert_eq!(shop_price(CardRarity::Epic, b) % 5, 0);
        }
    }

    #[test]
    fn stages_walk_the_campaign_and_stop_at_its_end() {
        assert_eq!(stage_zone(0, 0), 0);
        assert_eq!(stage_zone(0, 2), 2);
        assert_eq!(stage_zone(MAPS.len() - 2, 2), MAPS.len() - 1);
        assert!(STAGES >= 2);
        assert!(STAGE_LENGTH > 0.0 && STAGE_LENGTH <= 1.0);
    }

    #[test]
    fn rarities_draw_by_their_odds_and_rise_in_worth() {
        let mut seen = [0u32; 5];
        for i in 0..10_000 {
            seen[CardRarity::draw(i as f32 / 10_000.0) as usize] += 1;
        }
        for (r, n) in CardRarity::ALL.iter().zip(seen) {
            let want = r.odds() * 100.0;
            assert!((n as f32 - want).abs() <= 2.0, "{r:?}: {n} for {want}");
        }
        assert_eq!(CardRarity::draw(0.0), CardRarity::Common);
        assert_eq!(CardRarity::draw(1.0), CardRarity::Legendary);
        for w in CardRarity::ALL.windows(2) {
            assert!(w[0].weight() < w[1].weight());
            assert!(w[0].odds() > w[1].odds());
        }
        assert_eq!(
            CardRarity::Common.weight(),
            1.0,
            "a Common rank is a rank, as before"
        );
        for r in CardRarity::ALL {
            assert_eq!(CardRarity::from_wire(r.to_wire()), Some(r));
        }
        assert_eq!(CardRarity::from_wire(9), None);
    }

    #[test]
    fn passives_grow_by_a_positive_step() {
        for p in PASSIVES {
            assert!(p.per_level > 0.0, "{}: per_level {}", p.key, p.per_level);
        }
    }

    /// `spawn_enemies` indexes `[lo, lo + 1, lo + 2]` from `wave_pool`: a roster shorter than
    /// three would read out of bounds.
    #[test]
    fn every_zone_roster_has_at_least_three_tiers_and_a_boss() {
        for (i, m) in MAPS.iter().enumerate() {
            let r = roster(i);
            assert!(
                r.enemies.len() >= 3,
                "{}: {} tiers",
                m.name,
                r.enemies.len()
            );
            assert!(!r.bosses.is_empty(), "{}: no boss", m.name);
        }
    }

    /// One boss per slot exactly: a missing one would leave the run without its final boss, and a
    /// spare one would never come.
    #[test]
    fn every_zone_has_a_boss_for_each_slot_of_the_run() {
        for (i, m) in MAPS.iter().enumerate() {
            let n = roster(i).bosses.len();
            assert_eq!(n, BOSS_TIMES.len(), "{}: {n} bosses", m.name);
        }
    }

    /// The twelve zones of the campaign, in the order the main screen opens them.
    #[test]
    fn the_campaign_is_twelve_zones_in_order() {
        let names: Vec<&str> = MAPS.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            [
                "Elwynn Forest",
                "Duskwood",
                "Warsong Gulch",
                "Un'Goro Crater",
                "Arathi Basin",
                "Feralas",
                "Gurubashi Arena",
                "Scarlet Monastery",
                "Alterac Valley",
                "Blackrock Mountain",
                "Gates of Ahn'Qiraj",
                "Naxxramas",
            ]
        );
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
    }

    /// The final bosses the main screen names, each one opening the next zone.
    #[test]
    fn each_zone_ends_on_its_final_boss() {
        let finals: Vec<&str> = (0..MAPS.len())
            .map(|i| roster(i).bosses.last().unwrap().name)
            .collect();
        assert_eq!(
            finals,
            [
                "Edwin VanCleef",
                "Morbent Fel",
                "Lord Kazzak",
                "King Mosh",
                "Lord Aliden Perenolde",
                "King Gordok",
                "Hakkar the Soulflayer",
                "High Inquisitor Whitemane",
                "Azuregos",
                "Nefarian",
                "C'Thun",
                "Kel'Thuzad",
            ]
        );
        assert_eq!(
            unlock_hint(10).as_deref(),
            Some("Defeat Nefarian in Blackrock Mountain")
        );
        assert_eq!(unlock_hint(0), None);
    }

    #[test]
    fn elwynn_reads_as_the_screen_does() {
        let r = roster(0);
        let bosses: Vec<(f32, &str)> = (0..r.bosses.len())
            .map(|k| (Rules::SOLO.boss_time(k), r.bosses[k].name))
            .collect();
        assert_eq!(
            bosses,
            [
                (180.0, "Hogger"),
                (360.0, "Mother Fang"),
                (540.0, "Rhahk'Zor"),
                (720.0, "Mr. Smite"),
                (960.0, "Edwin VanCleef"),
            ]
        );
        let foes: Vec<&str> = r.enemies.iter().map(|e| e.name).collect();
        assert_eq!(
            foes,
            [
                "Kobold Vermin",
                "Young Wolf",
                "Murloc Streamrunner",
                "Kobold Geomancer",
                "Defias Thug",
                "Riverpaw Brute",
                "Prowler",
                "Defias Rogue Wizard",
                "Defias Enforcer",
            ]
        );
        assert_eq!(MAPS[0].zone, "The Defias Uprising");
    }

    #[test]
    fn zones_open_one_after_another() {
        let none = |_: &str| false;
        assert!(unlocked(0, none));
        assert!(!unlocked(1, none));
        let elwynn = |z: &str| z == "Elwynn Forest";
        assert!(unlocked(1, elwynn));
        assert!(!unlocked(2, elwynn));
        // Winning a later zone does not skip the one before it.
        let naxx = |z: &str| z == "Gates of Ahn'Qiraj";
        assert!(unlocked(11, naxx));
        assert!(!unlocked(10, naxx));
        assert!(!unlocked(MAPS.len(), |_| true));
    }

    /// Each zone is harder than the one before, and its final boss the toughest thing in it.
    #[test]
    fn the_campaign_hardens_and_ends_on_its_toughest() {
        for i in 1..MAPS.len() {
            assert!(MAPS[i].might > MAPS[i - 1].might, "{}", MAPS[i].name);
            let (a, b) = (roster(i - 1), roster(i));
            assert!(b.bosses[4].hp > a.bosses[4].hp, "{}", MAPS[i].name);
        }
        for i in 0..MAPS.len() {
            let r = roster(i);
            let last = r.bosses.last().unwrap();
            for e in r.enemies.iter().chain(&r.bosses[..r.bosses.len() - 1]) {
                assert!(
                    last.hp > e.hp,
                    "{}: {} vs {}",
                    MAPS[i].name,
                    last.name,
                    e.name
                );
            }
        }
    }

    #[test]
    fn rosters_have_positive_numbers() {
        for (i, m) in MAPS.iter().enumerate() {
            let r = roster(i);
            for e in r.enemies.iter().chain(&r.bosses) {
                assert!(e.hp > 0.0, "{} / {}: hp {}", m.name, e.name, e.hp);
                assert!(e.speed > 0.0, "{} / {}: speed {}", m.name, e.name, e.speed);
                assert!(
                    e.damage > 0.0,
                    "{} / {}: damage {}",
                    m.name,
                    e.name,
                    e.damage
                );
                assert!(e.scale > 0.0, "{} / {}: scale {}", m.name, e.name, e.scale);
                assert!(e.xp > 0, "{} / {}: no xp", m.name, e.name);
            }
        }
    }

    /// `ladder` reads off the running peak, so no later tier is softer than an earlier one.
    #[test]
    fn laddered_rosters_never_get_softer() {
        for (i, m) in MAPS.iter().enumerate() {
            if m.foes.is_empty() {
                continue;
            }
            let r = roster(i);
            for list in [&r.enemies, &r.bosses] {
                for w in list.windows(2) {
                    assert!(
                        w[1].hp >= w[0].hp && w[1].damage >= w[0].damage && w[1].xp >= w[0].xp,
                        "{}: {} is softer than {}",
                        m.name,
                        w[1].name,
                        w[0].name
                    );
                }
            }
        }
    }

    #[test]
    fn ladder_spans_the_tuned_ladder_end_to_end() {
        let foes = [
            foe("a", 1, 1, 1.0, 1.0),
            foe("b", 2, 2, 1.0, 1.0),
            foe("c", 3, 3, 1.0, 1.0),
        ];
        let out = ladder(&foes, ENEMIES, 1.0);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].hp, ENEMIES[0].hp);
        let peak = ENEMIES.iter().map(|e| e.hp).fold(0.0f32, f32::max);
        assert_eq!(out[2].hp, peak);
        // A single foe takes the first rung rather than dividing by zero.
        let one = ladder(&foes[..1], ENEMIES, 1.0);
        assert_eq!(one[0].hp, ENEMIES[0].hp);
    }

    #[test]
    fn roster_clamps_an_out_of_range_map() {
        let last = roster(MAPS.len() - 1) as *const Roster;
        assert_eq!(roster(MAPS.len() + 10) as *const Roster, last);
    }

    #[test]
    fn wave_pool_stays_in_bounds_and_only_climbs() {
        for (i, _) in MAPS.iter().enumerate() {
            let tiers = roster(i).enemies.len();
            let mut prev = 0;
            let mut t = 0.0;
            while t <= FINAL_BOSS_SECS * 1.5 {
                let pool = wave_pool(t, tiers);
                assert!(pool[2] < tiers, "t={t}: {pool:?} out of {tiers}");
                assert!(
                    pool[0] >= prev,
                    "t={t}: pool went back from {prev} to {}",
                    pool[0]
                );
                prev = pool[0];
                t += 1.0;
            }
        }
    }

    #[test]
    fn wave_pool_opens_on_the_weakest_and_ends_on_the_strongest() {
        assert_eq!(wave_pool(0.0, 30), [0, 1, 2]);
        assert_eq!(wave_pool(FINAL_BOSS_SECS, 30), [27, 28, 29]);
    }

    #[test]
    fn crowd_grows_and_caps() {
        assert_eq!(target_alive(0.0), (CROWD_START * LULL_CROWD) as usize);
        let late = FINAL_BOSS_SECS * 10.0;
        assert!(target_alive(late) <= (CROWD_PEAK * SURGE_CROWD) as usize);
        // Within each kind of moment, the crowd only grows.
        let (mut lull, mut surge) = (0, 0);
        for s in 0..=900 {
            let t = s as f32;
            let n = target_alive(t);
            let prev = if surging(t) { &mut surge } else { &mut lull };
            assert!(n >= *prev, "t={t}");
            *prev = n;
        }
    }

    #[test]
    fn a_spells_area_duration_and_targets_stop_but_its_damage_does_not() {
        let mut p = Perks::default();
        for _ in 0..100 {
            p = p
                .with(Perk::Area(0.3))
                .with(Perk::Duration(0.3))
                .with(Perk::Targets(2))
                .with(Perk::Damage(0.3));
        }
        assert_eq!(p.area, MAX_PERK_AREA);
        assert_eq!(p.duration, MAX_PERK_DURATION);
        assert_eq!(p.targets, MAX_PERK_TARGETS);
        assert!(p.damage > 29.0);
    }

    #[test]
    fn an_ambush_ring_leaves_its_openings() {
        let a = ring_angles(60, 0.0);
        assert_eq!(a.len(), 60);
        let mut sorted: Vec<f32> = a.iter().map(|x| x.rem_euclid(std::f32::consts::TAU)).collect();
        sorted.sort_by(f32::total_cmp);
        let mut gaps: Vec<f32> = sorted.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.push(sorted[0] + std::f32::consts::TAU - sorted[59]);
        let wide = gaps.iter().filter(|g| **g >= RING_GAP).count();
        assert_eq!(wide, RING_GAPS, "{gaps:?}");
    }

    #[test]
    fn the_horde_surges_on_its_clock() {
        assert!(!surging(0.0) && !surging(WAVE_EVERY - SURGE_SECS - 0.1));
        assert!(surging(WAVE_EVERY - SURGE_SECS) && surging(WAVE_EVERY - 0.1));
        assert!(!surging(WAVE_EVERY) && surging(2.0 * WAVE_EVERY - 1.0));
        assert_eq!(next_surge_in(0.0), WAVE_EVERY - SURGE_SECS);
        assert_eq!(next_surge_in(WAVE_EVERY - 1.0), 0.0);
        assert!((next_surge_in(WAVE_EVERY + 3.0) - (WAVE_EVERY - SURGE_SECS - 3.0)).abs() < 1e-4);
        // A surge is thicker than a lull at the same point of the stage.
        let r = Rules::SOLO;
        assert!(r.target_alive(WAVE_EVERY - 1.0) > r.target_alive(WAVE_EVERY + 1.0));
        // The waves average out a little above the even crowd.
        let mean = (LULL_CROWD * (WAVE_EVERY - SURGE_SECS) + SURGE_CROWD * SURGE_SECS) / WAVE_EVERY;
        assert!(mean > 1.0 && mean < 1.15, "{mean}");
    }

    #[test]
    fn enemy_scaling_starts_at_one_and_ends_where_tuned() {
        assert_eq!(hp_scale(0.0), 1.0);
        assert_eq!(damage_scale(0.0), 1.0);
        assert!((hp_scale(FINAL_BOSS_SECS) - 1.75).abs() < 1e-5);
        assert!((damage_scale(FINAL_BOSS_SECS) - (1.0 + HORDE_DAMAGE_GROWTH)).abs() < 1e-5);
        // A negative clock (never expected) does not shrink enemies below their base.
        assert_eq!(hp_scale(-10.0), 1.0);
    }

    #[test]
    fn xp_curve_is_positive_and_never_drops() {
        assert_eq!(xp_to_next(1), 5.0);
        // Quicker early, as before from 19 on.
        assert_eq!(xp_to_next(2), 13.0);
        assert_eq!(xp_to_next(9), 69.0);
        assert_eq!(xp_to_next(10), 77.0);
        let mut prev = 0.0;
        for level in 1..=100 {
            let xp = xp_to_next(level);
            assert!(xp > 0.0, "level {level}: {xp}");
            assert!(xp >= prev, "level {level}: {xp} after {prev}");
            prev = xp;
        }
        // The steps where the slope changes.
        assert_eq!(xp_to_next(19), 185.0);
        assert_eq!(xp_to_next(20), 195.0);
        assert_eq!(xp_to_next(39), 442.0);
        assert_eq!(xp_to_next(40), 455.0);
    }

    #[test]
    fn solo_rules_keep_the_original_curves() {
        let r = Rules::SOLO;
        for s in (0..=1200).step_by(15) {
            let t = s as f32;
            assert_eq!(r.hp_scale(t), 1.0 + (t / FINAL_BOSS_SECS) * 0.75);
            assert_eq!(
                r.bosses_due(t),
                BOSS_TIMES.iter().filter(|&&b| b <= t).count() as u32
            );
        }
    }

    #[test]
    fn more_heroes_mean_a_tougher_bigger_horde() {
        let solo = Rules::SOLO;
        let four = Rules {
            heroes: 4,
            ..Rules::SOLO
        };
        assert!(four.hp_scale(100.0) > solo.hp_scale(100.0));
        assert!(four.target_alive(100.0) > solo.target_alive(100.0));
        let flat = Rules {
            scale_with_heroes: false,
            ..four
        };
        assert_eq!(flat.hp_scale(100.0), solo.hp_scale(100.0));
        assert_eq!(flat.target_alive(100.0), solo.target_alive(100.0));
    }

    #[test]
    fn the_crowd_never_passes_the_hard_cap() {
        let r = Rules {
            density: 4.0,
            heroes: 4,
            ..Rules::SOLO
        };
        assert_eq!(r.target_alive(r.duration), HARD_CROWD_CAP);
        let sparse = Rules {
            density: 0.25,
            ..Rules::SOLO
        };
        assert!(sparse.target_alive(0.0) >= 1);
    }

    #[test]
    fn custom_durations_scale_the_run_and_its_bosses() {
        let long = Rules {
            duration: 1920.0,
            ..Rules::SOLO
        };
        // Progress runs against the custom clock.
        assert!((long.progress(960.0) - 0.5).abs() < 1e-6);
        // Every boss comes twice as late; the final one at the run's length.
        assert_eq!(long.boss_time(0), 360.0);
        assert_eq!(long.boss_time(4), 1920.0);
        assert_eq!(long.bosses_due(1919.0), 4);
        assert_eq!(long.bosses_due(1920.0), 5);
        // Never more bosses than the roster has.
        assert_eq!(long.bosses_due(1.0e6), 5);
        let r = roster(0);
        let (first, final0) = long.boss(r, 0);
        let (last, final4) = long.boss(r, 4);
        assert_eq!(first.name, "Hogger");
        assert!(!final0);
        assert_eq!(last.name, "Edwin VanCleef");
        assert!(final4);
    }

    #[test]
    fn boss_schedule_fits_inside_the_run() {
        // In order, the final one last, at the run's length.
        assert!(BOSS_TIMES.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(*BOSS_TIMES.last().unwrap(), FINAL_BOSS_SECS);
        assert_eq!(Rules::SOLO.boss_time(4), FINAL_BOSS_SECS);
    }

    #[test]
    fn every_race_has_three_distinct_racials() {
        let mut keys = HashSet::new();
        for race in 1..=8u8 {
            let r = racials(race);
            for def in r {
                assert!(keys.insert(def.key), "racial key {} twice", def.key);
                assert!(!def.name.is_empty() && !def.text.is_empty(), "{}", def.key);
                assert!(def.spell != 0, "{} needs an icon", def.key);
                assert!(
                    !def.bonuses.is_empty() || def.proc.is_some(),
                    "{} does nothing",
                    def.key
                );
                if let Some(p) = def.proc {
                    assert!(p.every > p.duration && p.duration > 0.0, "{}", p.key);
                    assert!(!p.bonuses.is_empty(), "{}", p.key);
                }
            }
        }
    }

    #[test]
    fn the_troll_racials_read_as_the_screen_does() {
        let r = racials(8);
        let berserking = r[0].proc.expect("Berserking fires on its own");
        assert_eq!((berserking.every, berserking.duration), (35.0, 10.0));
        assert_eq!(r[1].bonuses, &[Bonus::flat(Stat::Regen, 1.5)]);
        assert_eq!(r[2].bonuses, &[Bonus::flat(Stat::Crit, 0.03)]);
    }

    #[test]
    fn every_race_has_classes_and_every_class_a_race() {
        for race in 1..=8u8 {
            let classes = race_classes(race);
            assert!(classes.len() >= 4, "race {race}");
            for id in classes {
                assert!(
                    CLASSES.iter().any(|c| c.id == *id),
                    "race {race}: class {id}"
                );
            }
        }
        for c in CLASSES {
            assert!(
                (1..=8u8).any(|r| race_classes(r).contains(&c.id)),
                "{} has no race",
                c.name
            );
        }
        // Vanilla's: no Orc paladin, no Tauren mage, Druids only for Night Elves and Tauren.
        assert!(!race_classes(2).contains(&2));
        assert!(!race_classes(6).contains(&8));
        let druids: Vec<u8> = (1..=8).filter(|r| race_classes(*r).contains(&11)).collect();
        assert_eq!(druids, [4, 6]);
    }

    #[test]
    fn sixty_items_five_per_zone_one_per_boss() {
        assert_eq!(ITEMS.len(), 60);
        let names: HashSet<&str> = ITEMS.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names.len(), ITEMS.len(), "item names are unique");
        for z in 0..MAPS.len() {
            let table = zone_items(z);
            assert_eq!(table.len(), 5, "{}", MAPS[z].name);
            for k in 0..5 {
                let i = boss_item(z, k).unwrap();
                assert_eq!(item(&i.name), Some(i));
                assert!(i.name.starts_with(roster(z).bosses[k].name));
            }
        }
        assert_eq!(boss_item(0, 4).unwrap().name, "Edwin VanCleef's Gauntlets");
        // Every slot comes up six times.
        for s in GearSlot::ALL {
            assert_eq!(ITEMS.iter().filter(|i| i.slot == s).count(), 6, "{s:?}");
        }
    }

    #[test]
    fn rarer_and_later_items_give_more() {
        for i in ITEMS.iter() {
            assert!(!i.bonuses.is_empty());
            assert_eq!(i.bonuses[0].stat, i.slot.bonus().stat);
        }
        let first = |z: usize, k: usize| match boss_item(z, k).unwrap().bonuses[0].amount {
            Amount::Flat(v) | Amount::Percent(v) => v,
        };
        // The same slot two zones on is stronger.
        assert!(first(2, 0) > first(0, 0));
        assert_eq!(boss_item(0, 0).unwrap().rarity, Rarity::Uncommon);
        assert_eq!(boss_item(0, 4).unwrap().rarity, Rarity::Epic);
        assert_eq!(boss_item(11, 4).unwrap().rarity, Rarity::Legendary);
    }

    #[test]
    fn the_horde_drops_rarely_and_only_from_the_zone() {
        assert!(horde_item(0, 0.5).is_none());
        let odds = 1.0 / HORDE_ITEM_ODDS as f32;
        for k in 0..50 {
            let roll = odds * (k as f32 / 50.0);
            let i = horde_item(3, roll).expect("inside the odds");
            assert_eq!(i.zone, 3);
        }
        assert!(horde_item(3, odds).is_none());
    }

    #[test]
    fn the_collection_gives_a_share_of_each_item_and_adds_up() {
        let helm = ITEMS.iter().find(|i| i.slot == GearSlot::Head).unwrap();
        let b = collection_bonus(helm);
        assert_eq!(b.stat, helm.bonuses[0].stat);
        let (Amount::Percent(full), Amount::Percent(part)) = (helm.bonuses[0].amount, b.amount)
        else {
            panic!("a helm gives a percent of health");
        };
        assert!((part - full * COLLECTION_SHARE).abs() < 1e-6);
        let all: Vec<Bonus> = ITEMS.iter().map(collection_bonus).collect();
        let sum = sum_bonuses(&all);
        // One line per stat and kind the slots give: ten slots, ten lines.
        assert_eq!(sum.len(), 10);
        let stats: HashSet<Stat> = sum.iter().map(|b| b.stat).collect();
        assert!(stats.contains(&Stat::Crit) && stats.contains(&Stat::MaxHp));
    }

    #[test]
    fn a_run_earns_its_gold() {
        assert_eq!(run_gold(0, 0, false, 1.0), 0);
        assert_eq!(run_gold(800, 5, true, 1.0), 100 + 125 + 100);
        assert_eq!(run_gold(80, 1, false, 1.5), 53);
    }

    #[test]
    fn difficulties_harden_and_pay_more() {
        assert_eq!(Rules::solo_at(0), Rules::SOLO);
        for w in DIFFICULTIES.windows(2) {
            assert!(w[1].health > w[0].health && w[1].damage > w[0].damage);
            assert!(w[1].reward > w[0].reward);
        }
        assert_eq!(Rules::solo_at(99).health, DIFFICULTIES[2].health);
        assert!(Rules::solo_at(1).hp_scale(0.0) > Rules::SOLO.hp_scale(0.0));
    }

    #[test]
    fn horde_chances_add_up_to_the_odds() {
        for z in 0..MAPS.len() {
            let sum: f32 = zone_items(z).iter().map(|i| horde_chance(i)).sum();
            assert!(
                (sum - 1.0 / HORDE_ITEM_ODDS as f32).abs() < 1e-7,
                "{}",
                MAPS[z].name
            );
        }
        let uncommon = boss_item(0, 0).unwrap();
        let epic = boss_item(0, 4).unwrap();
        assert!((horde_chance(uncommon) - 2.0 * horde_chance(epic)).abs() < 1e-9);
    }

    #[test]
    fn a_run_meets_more_of_the_horde_as_it_goes() {
        let early = met_horde(0, 0.0, &Rules::SOLO);
        let late = met_horde(0, FINAL_BOSS_SECS, &Rules::SOLO);
        assert!(early.len() >= 3);
        assert!(late.len() > early.len());
        assert_eq!(late.len(), roster(0).enemies.len());
        let (horde, bosses) = all_creatures();
        assert!(horde.len() > 100 && bosses.len() >= 55);
    }

    #[test]
    fn achievements_are_unique_and_pay() {
        let keys: HashSet<&str> = ACHIEVEMENTS.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys.len(), ACHIEVEMENTS.len());
        assert!(ACHIEVEMENTS
            .iter()
            .all(|a| a.gold > 0 && !a.name.is_empty()));
        let zones = ACHIEVEMENTS
            .iter()
            .filter(|a| matches!(a.goal, Goal::WinZone(_)))
            .count();
        assert_eq!(zones, MAPS.len());
        assert!(ACHIEVEMENTS
            .iter()
            .any(|a| a.goal == Goal::Collect(ITEMS.len())));
    }

    #[test]
    fn bonuses_cross_the_wire_whole() {
        for b in [
            Bonus::flat(Stat::MaxHp, 25.0),
            Bonus::pct(Stat::Magnet, 0.08),
            Bonus::flat(Stat::Crit, 0.0125),
            Bonus::flat(Stat::Gold, -0.5),
        ] {
            let back = bonus_from_wire(bonus_to_wire(&b)).unwrap();
            assert_eq!(back.stat, b.stat);
            let (Amount::Flat(x) | Amount::Percent(x), Amount::Flat(y) | Amount::Percent(y)) =
                (back.amount, b.amount);
            assert!((x - y).abs() < 1e-4);
            assert_eq!(
                matches!(back.amount, Amount::Flat(_)),
                matches!(b.amount, Amount::Flat(_))
            );
        }
        assert!(bonus_from_wire((99, 0, 1)).is_none());
        let huge = bonus_from_wire((0, 0, i32::MAX)).unwrap();
        assert_eq!(huge.amount, Amount::Flat(WIRE_BONUS_CAP));
    }

    #[test]
    fn chests_cost_more_each_time() {
        assert_eq!(chest_cost(0), 20);
        assert_eq!(chest_cost(1), 30);
        for n in 0..20 {
            assert!(chest_cost(n + 1) >= chest_cost(n));
            assert_eq!(chest_cost(n) % 5, 0);
        }
        assert_eq!(chest_reward(0.0), Reward::RunItem);
        assert_eq!(chest_reward(0.7), Reward::SpellRank);
        assert!(matches!(chest_reward(0.99), Reward::Gear { .. }));
    }

    #[test]
    fn run_items_and_elites_hold_together() {
        let keys: HashSet<&str> = RUN_ITEMS.iter().map(|i| i.key).collect();
        assert_eq!(keys.len(), RUN_ITEMS.len());
        assert!(RUN_ITEMS.iter().all(|i| run_item(i.key) == Some(i)));
        for z in 0..MAPS.len() {
            let top = roster(z).enemies.last().unwrap().hp;
            for k in 0..ELITES.len() {
                let e = elite(z, k);
                assert!(e.hp > top * 4.0, "{}", e.name);
                assert!(e.name.ends_with(ELITES[k].title));
            }
        }
        // Every kind of reward is on the field.
        let kinds: Vec<Reward> = ELITES.iter().map(|e| e.reward).collect();
        assert!(kinds.contains(&Reward::RunItem) && kinds.contains(&Reward::SpellRank));
        assert!(kinds.iter().any(|r| matches!(r, Reward::Gear { .. })));
    }
}
