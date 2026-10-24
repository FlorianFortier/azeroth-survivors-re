//! The hero as a player character: the race's own body, the five appearance dials of character
//! creation (skin, face, hair style, hair colour, facial hair), and the items it wears, shown on
//! the body as a player's are.
//!
//! The client dresses a player from item entries (`PLAYER_VISIBLE_ITEM_*`) resolved through item
//! templates, which a server would answer. The mode has no server, so each worn piece gets an
//! entry of its own that names its display ([`entry`]), and every machine answers the template
//! from the entry alone ([`template`]): a guest dresses the host's heroes with nothing more than
//! their fields. The hero wears the class's starting outfit from `CharStartOutfit.dbc` and its
//! trainer's weapons, and over them the gear it has equipped from the runs' loot. A piece of gear
//! has no model of its own in the tables: it wears a display picked from the client's own
//! `ItemDisplayInfo.dbc` by the slot its icon names (`INV_Helmet_…`, `INV_Boots_…`), the same on
//! every machine ([`gear_worn`]); a weapon keeps the shape of the class's own, so a mage still
//! swings a staff.
//!
//! `WOW_HERO_BODY=trainer` (`?hero_body=trainer`) brings back the old hero, a class trainer's
//! creature look with its outfit baked in.

use benilla_formats::{CharCreateCatalog, ItemDisplayCatalog, StartOutfitItem};
use benilla_protocol::messages::ItemInfo;
use bevy::prelude::*;

use super::data::{ClassDef, GearSlot, ItemDef};

/// The five appearance dials, `[skin, face, hair style, hair colour, facial hair]`, each an index
/// into the (race, sex)'s own range.
pub(crate) type Appearance = [u8; 5];

/// The dials' names, as the main screen labels them.
pub(super) const DIALS: [&str; 5] = ["Skin", "Face", "Hair", "Hair Color", "Facial Hair"];

/// The equipment slots (`EQUIPMENT_SLOT_*`, the `PLAYER_VISIBLE_ITEM_*` index) a worn piece fills.
pub(super) mod slot {
    pub const HEAD: u8 = 0;
    pub const SHOULDERS: u8 = 2;
    pub const SHIRT: u8 = 3;
    pub const CHEST: u8 = 4;
    pub const WAIST: u8 = 5;
    pub const LEGS: u8 = 6;
    pub const FEET: u8 = 7;
    pub const WRISTS: u8 = 8;
    pub const HANDS: u8 = 9;
    pub const BACK: u8 = 14;
    pub const MAIN_HAND: u8 = 15;
    pub const OFF_HAND: u8 = 16;
    pub const RANGED: u8 = 17;
    pub const TABARD: u8 = 18;
}

/// The ten slots the main screen shows: their name, the paper doll's empty-slot art, the slot.
pub(super) const SHOWN: [(&str, &str, u8); 10] = [
    ("Head", "Head", slot::HEAD),
    ("Shoulders", "Shoulder", slot::SHOULDERS),
    ("Chest", "Chest", slot::CHEST),
    ("Wrists", "Wrists", slot::WRISTS),
    ("Hands", "Hands", slot::HANDS),
    ("Waist", "Waist", slot::WAIST),
    ("Legs", "Legs", slot::LEGS),
    ("Feet", "Feet", slot::FEET),
    ("Main Hand", "MainHand", slot::MAIN_HAND),
    ("Off Hand", "SecondaryHand", slot::OFF_HAND),
];

/// One piece the hero wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Worn {
    pub slot: u8,
    /// `ItemDisplayInfo.dbc` id.
    pub display: u32,
    /// `InventoryType`.
    pub inv: u8,
    /// A weapon's item class (2), else armour (4); with its subclass, sheath type and material,
    /// what the draw, swing and stow animations and sounds read.
    pub class: u8,
    pub subclass: u8,
    pub sheath: u8,
    pub material: u8,
}

impl Worn {
    /// An armour piece.
    fn armour(slot: u8, display: u32, inv: u8) -> Self {
        Self {
            slot,
            display,
            inv,
            class: 4,
            subclass: 0,
            sheath: 0,
            material: 0,
        }
    }

    /// A class trainer's weapon, `data::Weapon`.
    fn weapon(w: &super::data::Weapon) -> Self {
        let &(hand, display, class, subclass, material, inv, sheath) = w;
        Self {
            slot: match hand {
                0 => slot::MAIN_HAND,
                1 => slot::OFF_HAND,
                _ => slot::RANGED,
            },
            display,
            inv,
            class,
            subclass,
            sheath,
            material,
        }
    }
}

/// The equipment slot a [`GearSlot`] fills, and an armour piece's inventory type there.
pub(super) fn gear_slot(s: GearSlot) -> (u8, u8) {
    match s {
        GearSlot::Head => (slot::HEAD, 1),
        GearSlot::Shoulders => (slot::SHOULDERS, 3),
        GearSlot::Chest => (slot::CHEST, 5),
        GearSlot::Wrists => (slot::WRISTS, 9),
        GearSlot::Hands => (slot::HANDS, 10),
        GearSlot::Waist => (slot::WAIST, 6),
        GearSlot::Legs => (slot::LEGS, 7),
        GearSlot::Feet => (slot::FEET, 8),
        GearSlot::MainHand => (slot::MAIN_HAND, 13),
        GearSlot::OffHand => (slot::OFF_HAND, 23),
    }
}

/// The equipment slot an armour piece's inventory type fills; `None` for a weapon (the hero's come
/// from its class) or anything not shown on the body.
fn armour_slot(inv: u8) -> Option<u8> {
    Some(match inv {
        1 => slot::HEAD,
        3 => slot::SHOULDERS,
        4 => slot::SHIRT,
        5 | 20 => slot::CHEST,
        6 => slot::WAIST,
        7 => slot::LEGS,
        8 => slot::FEET,
        9 => slot::WRISTS,
        10 => slot::HANDS,
        16 => slot::BACK,
        19 => slot::TABARD,
        _ => return None,
    })
}

/// What a hero of `class` wears: the starting outfit's armour, then the class's weapons.
pub(super) fn outfit(class: &ClassDef, start: &[StartOutfitItem]) -> Vec<Worn> {
    let mut out: Vec<Worn> = Vec::new();
    for item in start {
        let Some(slot) = armour_slot(item.inv_type) else {
            continue;
        };
        if item.display_id == 0 || out.iter().any(|w| w.slot == slot) {
            continue;
        }
        out.push(Worn::armour(slot, item.display_id, item.inv_type));
    }
    for w in class.weapons {
        let worn = Worn::weapon(w);
        if worn.display != 0 && !out.iter().any(|o| o.slot == worn.slot) {
            out.push(worn);
        }
    }
    out
}

/// The icon-name stems an armour slot's displays carry.
fn armour_stems(s: GearSlot) -> &'static [&'static str] {
    match s {
        GearSlot::Head => &["inv_helmet_"],
        GearSlot::Shoulders => &["inv_shoulder_"],
        GearSlot::Chest => &["inv_chest_"],
        GearSlot::Wrists => &["inv_bracer_"],
        GearSlot::Hands => &["inv_gauntlets_"],
        GearSlot::Waist => &["inv_belt_"],
        GearSlot::Legs => &["inv_pants_"],
        GearSlot::Feet => &["inv_boots_"],
        GearSlot::MainHand | GearSlot::OffHand => &[],
    }
}

/// The icon-name stems of a weapon's family, by item class and subclass.
fn weapon_stems(class: u8, subclass: u8) -> &'static [&'static str] {
    match (class, subclass) {
        (2, 0 | 1) => &["inv_axe_"],
        (2, 4 | 5) => &["inv_mace_", "inv_hammer_"],
        (2, 6) => &["inv_spear_"],
        (2, 7 | 8) => &["inv_sword_"],
        (2, 10) => &["inv_staff_"],
        (2, 15) => &["inv_weapon_shortblade_"],
        (2, 2) => &["inv_weapon_bow_"],
        (2, 3) => &["inv_weapon_rifle_"],
        (2, 18) => &["inv_weapon_crossbow_"],
        (2, 19) => &["inv_wand_"],
        (4, 6) => &["inv_shield_"],
        (4, 0) => &["inv_misc_book_", "inv_misc_orb_"],
        _ => &[],
    }
}

/// The displays whose icon names one of `stems`, in id order: the same list on every machine.
fn pool(catalog: &ItemDisplayCatalog, stems: &[&str]) -> Vec<u32> {
    let mut out: Vec<u32> = catalog
        .iter()
        .filter(|(_, d)| {
            d.icon.as_deref().is_some_and(|icon| {
                let base = icon.rsplit(['\\', '/']).next().unwrap_or(icon).to_ascii_lowercase();
                stems.iter().any(|s| base.starts_with(s))
            })
        })
        .map(|(id, _)| id)
        .collect();
    out.sort_unstable();
    out
}

/// A name's stable pick among `n` (FNV-1a), the same on every machine and every run.
fn pick(name: &str, n: usize) -> usize {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    (h as usize) % n.max(1)
}

/// How a piece of gear looks on a hero of `class`: an armour piece wears a display of its slot,
/// a weapon or off-hand one of the class's own weapon family, numbered as the class's. `None`
/// when the client has no display for it (an off-hand for a class that carries none).
pub(super) fn gear_worn(
    item: &ItemDef,
    class: &ClassDef,
    catalog: &ItemDisplayCatalog,
) -> Option<Worn> {
    let (slot, inv) = gear_slot(item.slot);
    let base = match item.slot {
        GearSlot::MainHand | GearSlot::OffHand => {
            let hand = if item.slot == GearSlot::MainHand { 0 } else { 1 };
            Worn::weapon(class.weapons.iter().find(|w| w.0 == hand)?)
        }
        _ => Worn::armour(slot, 0, inv),
    };
    let stems = match item.slot {
        GearSlot::MainHand | GearSlot::OffHand => weapon_stems(base.class, base.subclass),
        s => armour_stems(s),
    };
    let displays = pool(catalog, stems);
    if displays.is_empty() {
        return None;
    }
    Some(Worn {
        display: displays[pick(&item.name, displays.len())],
        ..base
    })
}

/// The icon a piece of gear shows on the main screen: its look on the class, or any display of
/// its slot for one the class does not show.
pub(super) fn gear_icon(
    item: &ItemDef,
    class: Option<&ClassDef>,
    catalog: &ItemDisplayCatalog,
) -> Option<String> {
    let display = class
        .and_then(|c| gear_worn(item, c, catalog))
        .map(|w| w.display)
        .or_else(|| {
            let stems: &[&str] = match item.slot {
                GearSlot::MainHand => &["inv_sword_"],
                GearSlot::OffHand => &["inv_shield_"],
                s => armour_stems(s),
            };
            let displays = pool(catalog, stems);
            (!displays.is_empty()).then(|| displays[pick(&item.name, displays.len())])
        })?;
    catalog.get(display)?.icon.clone()
}

/// The top bit marks the mode's own item entries; no real template has one.
const OWN: u32 = 0x8000_0000;
/// The entry's weapon bit: item class 2 rather than 4.
const WEAPON: u32 = 0x4000_0000;
/// The entry's wood bit: a wooden haft's draw sound rather than metal's.
const WOOD: u32 = 0x2000_0000;

/// The item entry a worn piece goes by, readable back by [`template`] on any machine: the display
/// in the low 16 bits (`ItemDisplayInfo` ids stay below them), then the inventory type, subclass,
/// sheath type, wood and weapon bits.
pub(super) fn entry(w: &Worn) -> u32 {
    let mut e = OWN
        | (w.display & 0xffff)
        | u32::from(w.inv & 0x1f) << 16
        | u32::from(w.subclass & 0x1f) << 21
        | u32::from(w.sheath & 0x7) << 26;
    if w.class == 2 {
        e |= WEAPON;
    }
    if w.material == 2 {
        e |= WOOD;
    }
    e
}

/// Whether an entry is one of the mode's own.
pub(super) fn is_own(entry: u32) -> bool {
    entry & OWN != 0
}

/// The template for one of the mode's own entries: an armour piece, or a weapon numbered as the
/// class trainer who carries it (its class, subclass, material and sheath, which the draw, swing
/// and stow animations read).
pub(super) fn template(entry: u32) -> Option<ItemInfo> {
    if !is_own(entry) {
        return None;
    }
    let display = entry & 0xffff;
    let inv = (entry >> 16) & 0x1f;
    let subclass = (entry >> 21) & 0x1f;
    let sheath = (entry >> 26) & 0x7;
    let class = if entry & WEAPON != 0 { 2 } else { 4 };
    let material = if entry & WOOD != 0 {
        2
    } else if class == 2 || sheath != 0 {
        1
    } else {
        0
    };
    Some(ItemInfo {
        class,
        subclass,
        name: String::from("Starting gear"),
        display_info_id: display,
        quality: 1,
        flags: 0,
        buy_price: 0,
        sell_price: 0,
        inventory_type: inv,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        required_skill: 0,
        required_skill_rank: 0,
        required_spell: 0,
        required_honor_rank: 0,
        required_city_rank: 0,
        required_rep_faction: 0,
        required_rep_rank: 0,
        max_count: 0,
        stackable: 1,
        container_slots: 0,
        stats: Vec::new(),
        damages: Vec::new(),
        dmg_min: 0.0,
        dmg_max: 0.0,
        dmg_type: 0,
        armor: 0,
        resistances: [0; 6],
        delay_ms: 0,
        ammo_type: 0,
        ranged_mod_range: 0.0,
        spells: Vec::new(),
        spell_charges_0: 0,
        use_spell: None,
        bonding: 0,
        description: String::new(),
        page_text: 0,
        language_id: 0,
        page_material: 0,
        start_quest: 0,
        lock_id: 0,
        material,
        sheath,
        random_property: 0,
        block: 0,
        item_set: 0,
        max_durability: 0,
        area: 0,
        map: 0,
        bag_family: 0,
    })
}

/// The `PLAYER_VISIBLE_ITEM_n_0` field of an equipment slot.
pub(super) fn visible_field(slot: u8) -> u16 {
    260 + 12 * u16::from(slot)
}

/// `PLAYER_BYTES` and `PLAYER_BYTES_2` for the dials.
pub(super) fn appearance_fields(a: &Appearance) -> [(u16, u32); 2] {
    let bytes =
        u32::from(a[0]) | u32::from(a[1]) << 8 | u32::from(a[2]) << 16 | u32::from(a[3]) << 24;
    [(193, bytes), (194, u32::from(a[4]))]
}

/// The dials kept inside the (race, sex)'s ranges.
pub(super) fn clamp(a: Appearance, counts: [u8; 5]) -> Appearance {
    let mut out = a;
    for (d, &n) in out.iter_mut().zip(&counts) {
        *d = if n == 0 { 0 } else { (*d).min(n - 1) };
    }
    out
}

/// A (race, sex)'s dial counts, or one of each without the catalog.
pub(super) fn counts(catalog: Option<&CharCreateCatalog>, race: u8, sex: u8) -> [u8; 5] {
    catalog
        .and_then(|c| c.ranges(race, sex))
        .map(|r| [r.skin, r.face, r.hair_style, r.hair_color, r.facial_hair])
        .unwrap_or([1; 5])
}

/// Whether the hero is the old trainer look (`WOW_HERO_BODY=trainer`).
fn trainer_body() -> bool {
    static TRAINER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRAINER.get_or_init(|| {
        crate::webenv::var("WOW_HERO_BODY")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("trainer"))
    })
}

/// How a hero is drawn.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Body {
    /// The race's own body, dressed piece by piece.
    Player {
        display: u32,
        appearance: Appearance,
        worn: Vec<Worn>,
    },
    /// A class trainer's creature look, its outfit baked in; without the catalog, or asked for.
    Trainer { display: u32 },
}

/// The body of a hero of `class`, race and sex, with the dials picked and `gear` equipped over
/// the starting outfit; `trainer` is the class trainer's display the old look used.
#[allow(clippy::too_many_arguments)]
pub(super) fn body(
    class: &ClassDef,
    race: u8,
    sex: u8,
    appearance: Appearance,
    trainer: u32,
    gear: &[&ItemDef],
    catalog: Option<&CharCreateCatalog>,
    displays: Option<&ItemDisplayCatalog>,
) -> Body {
    let player = catalog
        .filter(|_| !trainer_body())
        .and_then(|c| Some((c, c.body_display(race, sex)?)));
    match player {
        Some((c, display)) => {
            let mut worn = outfit(class, c.start_outfit(race, class.id, sex));
            if let Some(displays) = displays {
                for item in gear {
                    let Some(w) = gear_worn(item, class, displays) else {
                        continue;
                    };
                    worn.retain(|o| o.slot != w.slot);
                    worn.push(w);
                }
            }
            Body::Player {
                display,
                appearance: clamp(appearance, counts(Some(c), race, sex)),
                worn,
            }
        }
        None => Body::Trainer { display: trainer },
    }
}

/// Answer the templates of the mode's own entries among `entries` before anything asks for them.
pub(super) fn queue_templates(commands: &mut Commands, entries: Vec<u32>) {
    let own: Vec<u32> = entries.into_iter().filter(|e| is_own(*e)).collect();
    if own.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        let Some(mut items) = world.get_resource_mut::<crate::items::Items>() else {
            return;
        };
        for e in own {
            if items.template_cached(e).is_none() {
                items.insert_template(e, template(e));
            }
        }
    });
}

/// The entries a hero's fields wear, for [`queue_templates`].
pub(super) fn worn_entries(fields: &[(u16, u32)]) -> Vec<u32> {
    fields
        .iter()
        .filter(|(i, _)| (260..260 + 12 * 19).contains(i) && (i - 260) % 12 == 0)
        .map(|&(_, v)| v)
        .filter(|v| is_own(*v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::data;

    fn warrior() -> &'static ClassDef {
        data::CLASSES.iter().find(|c| c.id == 1).unwrap()
    }

    #[test]
    fn an_entry_reads_back_as_its_piece() {
        let w = Worn::armour(slot::CHEST, 9891, 20);
        let e = entry(&w);
        assert!(is_own(e));
        let t = template(e).unwrap();
        assert_eq!(
            (t.display_info_id, t.inventory_type, t.class),
            (9891, 20, 4)
        );
        // A real template's entry is never taken for one of ours.
        assert!(template(2361).is_none());
    }

    #[test]
    fn a_class_weapon_keeps_its_animation_numbers() {
        let c = warrior();
        let &(hand, display, class, sub, material, inv, sheath) = &c.weapons[0];
        assert_eq!(hand, 0);
        let worn = outfit(c, &[]);
        let main = worn.iter().find(|w| w.slot == slot::MAIN_HAND).unwrap();
        let t = template(entry(main)).unwrap();
        assert_eq!(t.display_info_id, display);
        assert_eq!(
            (t.class, t.subclass, t.material, t.sheath, t.inventory_type),
            (
                u32::from(class),
                u32::from(sub),
                u32::from(material),
                u32::from(sheath),
                u32::from(inv)
            )
        );
    }

    #[test]
    fn the_starting_outfit_fills_its_slots_once() {
        let start = [
            StartOutfitItem {
                display_id: 100,
                inv_type: 4,
            },
            StartOutfitItem {
                display_id: 101,
                inv_type: 20,
            },
            StartOutfitItem {
                display_id: 102,
                inv_type: 5,
            },
            StartOutfitItem {
                display_id: 103,
                inv_type: 7,
            },
            // A weapon in the outfit gives way to the class's own.
            StartOutfitItem {
                display_id: 104,
                inv_type: 13,
            },
            StartOutfitItem {
                display_id: 0,
                inv_type: 8,
            },
        ];
        let worn = outfit(warrior(), &start);
        let at = |s: u8| worn.iter().filter(|w| w.slot == s).count();
        assert_eq!(at(slot::SHIRT), 1);
        assert_eq!(at(slot::CHEST), 1);
        assert_eq!(
            worn.iter().find(|w| w.slot == slot::CHEST).unwrap().display,
            101
        );
        assert_eq!(at(slot::LEGS), 1);
        assert_eq!(at(slot::FEET), 0);
        assert!(!worn.iter().any(|w| w.display == 104));
        assert_eq!(at(slot::MAIN_HAND), 1);
    }

    #[test]
    fn fields_carry_the_dials_and_the_worn_entries() {
        let [(i, b), (j, b2)] = appearance_fields(&[1, 2, 3, 4, 5]);
        assert_eq!((i, j), (193, 194));
        assert_eq!(b, 0x0403_0201);
        assert_eq!(b2, 5);
        let w = Worn::armour(slot::HEAD, 7, 1);
        let fields = vec![
            (visible_field(slot::HEAD), entry(&w)),
            (visible_field(slot::HEAD) + 1, 99),
            (36, 0x0101),
        ];
        assert_eq!(worn_entries(&fields), [entry(&w)]);
        assert_eq!(visible_field(slot::MAIN_HAND), 260 + 12 * 15);
    }

    #[test]
    fn every_class_weapon_reads_back_whole() {
        for c in data::CLASSES {
            for w in c.weapons {
                let worn = Worn::weapon(w);
                let t = template(entry(&worn)).unwrap();
                assert_eq!(t.display_info_id, worn.display, "{}", c.name);
                assert_eq!(
                    (t.class, t.subclass, t.sheath, t.inventory_type),
                    (
                        u32::from(worn.class),
                        u32::from(worn.subclass),
                        u32::from(worn.sheath),
                        u32::from(worn.inv)
                    ),
                    "{}",
                    c.name
                );
            }
        }
    }

    fn catalog() -> ItemDisplayCatalog {
        let row = |icon: &str| benilla_formats::ItemDisplay {
            icon: Some(format!("Interface\\Icons\\{icon}")),
            ..Default::default()
        };
        let mut rows = std::collections::HashMap::new();
        rows.insert(10, row("INV_Helmet_01"));
        rows.insert(11, row("INV_Helmet_02"));
        rows.insert(20, row("INV_Boots_01"));
        rows.insert(30, row("INV_Sword_04"));
        rows.insert(31, row("INV_Staff_08"));
        rows.insert(32, row("INV_Mace_01"));
        rows.insert(40, row("INV_Shield_05"));
        ItemDisplayCatalog::from_displays(rows)
    }

    #[test]
    fn gear_wears_a_display_of_its_slot_the_same_every_time() {
        let cat = catalog();
        let helm = data::ITEMS
            .iter()
            .find(|i| i.slot == GearSlot::Head)
            .unwrap();
        let w = gear_worn(helm, warrior(), &cat).unwrap();
        assert!([10, 11].contains(&w.display));
        assert_eq!((w.slot, w.inv, w.class), (slot::HEAD, 1, 4));
        assert_eq!(gear_worn(helm, warrior(), &cat), Some(w));
        let boots = data::ITEMS
            .iter()
            .find(|i| i.slot == GearSlot::Feet)
            .unwrap();
        assert_eq!(gear_worn(boots, warrior(), &cat).unwrap().display, 20);
    }

    #[test]
    fn a_weapon_keeps_the_class_shape() {
        let cat = catalog();
        let blade = data::ITEMS
            .iter()
            .find(|i| i.slot == GearSlot::MainHand)
            .unwrap();
        for c in data::CLASSES {
            let Some(&main) = c.weapons.iter().find(|w| w.0 == 0) else {
                continue;
            };
            let Some(w) = gear_worn(blade, c, &cat) else {
                continue;
            };
            assert_eq!((w.class, w.subclass, w.sheath), (main.2, main.3, main.6), "{}", c.name);
            if (main.2, main.3) == (2, 10) {
                assert_eq!(w.display, 31, "{} swings a staff", c.name);
            }
        }
        // A class with no off-hand shows none.
        let ward = data::ITEMS
            .iter()
            .find(|i| i.slot == GearSlot::OffHand)
            .unwrap();
        for c in data::CLASSES.iter().filter(|c| !c.weapons.iter().any(|w| w.0 == 1)) {
            assert!(gear_worn(ward, c, &cat).is_none(), "{}", c.name);
            assert!(gear_icon(ward, Some(c), &cat).is_some());
        }
    }

    #[test]
    fn dials_stay_in_range() {
        assert_eq!(clamp([9, 9, 9, 9, 9], [3, 0, 10, 2, 1]), [2, 0, 9, 1, 0]);
        assert_eq!(counts(None, 1, 0), [1; 5]);
    }

    #[test]
    fn the_shown_slots_are_the_screens_ten() {
        assert_eq!(SHOWN.len(), 10);
        let slots: std::collections::HashSet<u8> = SHOWN.iter().map(|s| s.2).collect();
        assert_eq!(slots.len(), 10);
    }
}
