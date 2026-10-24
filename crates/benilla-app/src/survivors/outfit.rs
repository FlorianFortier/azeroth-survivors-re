//! The hero as a player character: the race's own body, the five appearance dials of character
//! creation (skin, face, hair style, hair colour, facial hair), and the items it wears, shown on
//! the body as a player's are.
//!
//! The client dresses a player from item entries (`PLAYER_VISIBLE_ITEM_*`) resolved through item
//! templates, which a server would answer. The mode has no server, so each worn piece gets an
//! entry of its own that names its display ([`entry`]), and every machine answers the template
//! from the entry alone ([`template`]): a guest dresses the host's heroes with nothing more than
//! their fields. Today the hero wears the class's starting outfit from `CharStartOutfit.dbc` and
//! its trainer's weapons; the loot of the runs replaces them as it lands.
//!
//! `WOW_HERO_BODY=trainer` (`?hero_body=trainer`) brings back the old hero, a class trainer's
//! creature look with its outfit baked in.

use benilla_formats::{CharCreateCatalog, StartOutfitItem};
use benilla_protocol::messages::ItemInfo;
use bevy::prelude::*;

use super::data::{self, ClassDef};

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
        out.push(Worn {
            slot,
            display: item.display_id,
            inv: item.inv_type,
        });
    }
    for &(hand, display, _, _, _, inv, _) in class.weapons {
        let slot = match hand {
            0 => slot::MAIN_HAND,
            1 => slot::OFF_HAND,
            _ => slot::RANGED,
        };
        if display != 0 && !out.iter().any(|w| w.slot == slot) {
            out.push(Worn { slot, display, inv });
        }
    }
    out
}

/// The top bit marks the mode's own item entries; no real template has one.
const OWN: u32 = 0x8000_0000;

/// The item entry a worn piece goes by: its inventory type and display, readable back by
/// [`template`] on any machine.
pub(super) fn entry(w: &Worn) -> u32 {
    OWN | u32::from(w.inv & 0x7f) << 24 | (w.display & 0x00ff_ffff)
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
    let inv = (entry >> 24) & 0x7f;
    let display = entry & 0x00ff_ffff;
    let weapon = data::CLASSES
        .iter()
        .flat_map(|c| c.weapons.iter())
        .find(|w| w.1 == display && u32::from(w.5) == inv);
    let (class, subclass, material, sheath) = match weapon {
        Some(&(_, _, class, sub, material, _, sheath)) => (
            u32::from(class),
            u32::from(sub),
            u32::from(material),
            u32::from(sheath),
        ),
        None => (4, 0, 0, 0),
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

/// The body of a hero of `class`, race and sex, with the dials picked; `trainer` is the class
/// trainer's display the old look used.
pub(super) fn body(
    class: &ClassDef,
    race: u8,
    sex: u8,
    appearance: Appearance,
    trainer: u32,
    catalog: Option<&CharCreateCatalog>,
) -> Body {
    let player = catalog
        .filter(|_| !trainer_body())
        .and_then(|c| Some((c, c.body_display(race, sex)?)));
    match player {
        Some((c, display)) => Body::Player {
            display,
            appearance: clamp(appearance, counts(Some(c), race, sex)),
            worn: outfit(class, c.start_outfit(race, class.id, sex)),
        },
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

    fn warrior() -> &'static ClassDef {
        data::CLASSES.iter().find(|c| c.id == 1).unwrap()
    }

    #[test]
    fn an_entry_reads_back_as_its_piece() {
        let w = Worn {
            slot: slot::CHEST,
            display: 9891,
            inv: 20,
        };
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
        let w = Worn {
            slot: slot::HEAD,
            display: 7,
            inv: 1,
        };
        let fields = vec![
            (visible_field(slot::HEAD), entry(&w)),
            (visible_field(slot::HEAD) + 1, 99),
            (36, 0x0101),
        ];
        assert_eq!(worn_entries(&fields), [entry(&w)]);
        assert_eq!(visible_field(slot::MAIN_HAND), 260 + 12 * 15);
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
