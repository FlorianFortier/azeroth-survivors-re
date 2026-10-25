//! The mode's two languages: the English it is written in, and French. Text is written in
//! English and looked up here ([`tr`]); a template with `{}` holes is filled after its lookup
//! ([`trf`]). What the client's own data names (a spell's name and tooltip) follows the client's
//! language, not this one.
//!
//! The language is the profile's ([`super::profile::Options::language`]); changing it rebuilds
//! the main screen, whose refreshes watch [`stale`].

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

/// The languages, by the code the profile stores.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Lang {
    En,
    Fr,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Fr];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    /// The language's own name, as its button reads.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
        }
    }

    pub fn from_code(code: &str) -> Lang {
        match code.trim().to_ascii_lowercase().as_str() {
            "fr" | "fr-fr" | "french" | "français" => Lang::Fr,
            _ => Lang::En,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// The language text is shown in.
pub(super) fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 {
        Lang::Fr
    } else {
        Lang::En
    }
}

/// Show text in `lang` from now on; true when it changed.
pub(super) fn set(lang: Lang) -> bool {
    let v = u8::from(lang == Lang::Fr);
    if CURRENT.swap(v, Ordering::Relaxed) == v {
        return false;
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
    true
}

/// Whether the language changed since a refresh last looked (its `seen`): the refresh then draws
/// again.
pub(super) fn stale(seen: &mut u32) -> bool {
    let now = GENERATION.load(Ordering::Relaxed);
    if *seen == now {
        return false;
    }
    *seen = now;
    true
}

/// `s` in the current language: its French where the table has it, else as written.
pub(super) fn tr(s: &str) -> &str {
    tr_in(current(), s)
}

fn tr_in(lang: Lang, s: &str) -> &str {
    match lang {
        Lang::En => s,
        Lang::Fr => french(s).unwrap_or(s),
    }
}

/// A template in the current language, its `{}` holes filled in order.
pub(super) fn trf(template: &str, args: &[&str]) -> String {
    trf_in(current(), template, args)
}

fn trf_in(lang: Lang, template: &str, args: &[&str]) -> String {
    let t = tr_in(lang, template);
    let mut out = String::with_capacity(t.len() + 16);
    let mut args = args.iter();
    let mut rest = t;
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        out.push_str(args.next().copied().unwrap_or(""));
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// The French of an English text, `None` for one the table lacks.
fn french(s: &str) -> Option<&'static str> {
    Some(match s {
        // The main screen.
        "Character" => "Personnage",
        "Collection" => "Collection",
        "Drop Tables" => "Tables de butin",
        "Codex" => "Codex",
        "Talents" => "Talents",
        "Achievements" => "Hauts faits",
        "Rewards" => "Récompenses",
        "Rewards ({})" => "Récompenses ({})",
        "Options" => "Options",
        "Class" => "Classe",
        "Zone" => "Zone",
        "Equipment" => "Équipement",
        "Armory" => "Armurerie",
        "Every piece you own, slot by slot" => "Toutes vos pièces, emplacement par emplacement",
        "Spend your talent points" => "Dépensez vos points de talent",
        "Pick a class." => "Choisissez une classe.",
        "Health" => "Vie",
        "Starts with" => "Commence avec",
        "Zones open" => "Zones ouvertes",
        "{} of {}" => "{} sur {}",
        "SPELL POOL" => "SORTS DE LA CLASSE",
        "Random" => "Aléatoire",
        "Random look" => "Apparence aléatoire",
        "Male" => "Homme",
        "Female" => "Femme",
        "ALLIANCE" => "ALLIANCE",
        "HORDE" => "HORDE",
        "Fights for the Alliance." => "Combat pour l'Alliance.",
        "Fights for the Horde." => "Combat pour la Horde.",
        "Racials" => "Raciaux",
        "Racial" => "Racial",
        "Always on" => "Toujours actif",
        "Fires on its own" => "Se déclenche seul",
        "Play as a {}." => "Jouer un {}.",
        "Picking {} changes your class." => "Choisir {} change votre classe.",
        "Click for the next one." => "Cliquez pour le suivant.",
        "Skin" => "Peau",
        "Face" => "Visage",
        "Hair" => "Coiffure",
        "Hair Color" => "Couleur",
        "Facial Hair" => "Pilosité",
        "Starting gear" => "Équipement de départ",
        "Empty" => "Vide",
        "{} found: click to wear one." => "{} trouvé(s) : cliquez pour en porter un.",
        "Click to take it off." => "Cliquez pour l'enlever.",
        "Click for the next of the {} you own." => "Cliquez pour le suivant des {} possédés.",
        "Dropped by {}, {}" => "Lâché par {}, {}",
        "Run Options" => "Options de partie",
        "{} difficulty" => "Difficulté {}",
        "Difficulty" => "Difficulté",
        "A lobby plays by its host's rules." => "Un salon suit les règles de son hôte.",
        "Enter {}" => "Entrer : {}",
        "Defeat {} at {} · {} bosses on the way" => "Vaincre {} à {} · {} boss en chemin",
        "Defeat {} to win." => "Vaincre {} pour gagner.",
        "Locked · {}" => "Verrouillé · {}",
        "Defeat {} in {}" => "Vaincre {} : {}",
        "Co-op" => "Coopération",
        "Play with friends" => "Jouer entre amis",
        "Back to your lobby" => "Retour au salon",
        "WASD to run · the mouse to look · 1 2 3 picks a card · Enter to play" => {
            "ZQSD ou WASD pour courir · la souris pour regarder · 1 2 3 choisit une carte · Entrée pour jouer"
        }
        "Coming soon." => "Bientôt.",
        "Back" => "Retour",
        "None" => "Aucun",
        "Nothing found yet." => "Rien trouvé pour l'instant.",
        "What you wear shows on your hero and adds to every run." => {
            "Ce que vous portez se voit sur votre héros et compte à chaque partie."
        }
        "Worn · found {}×" => "Porté · trouvé {}×",
        "Click to wear it · found {}×" => "Cliquez pour le porter · trouvé {}×",
        "Find gear to grow your collection: every piece found adds a bonus to every run." => {
            "Trouvez de l'équipement : chaque pièce trouvée ajoute un bonus à chaque partie."
        }
        "{} of {} found" => "{} sur {} trouvés",
        "Not found yet" => "Pas encore trouvé",
        "Collected: +{} every run" => "Collectionné : +{} à chaque partie",
        "Found" => "Trouvé",
        "Gold" => "Or",
        "Points spent" => "Points dépensés",
        "Train a point" => "Acheter un point",
        "Every point is trained" => "Tous les points sont achetés",
        "{} gold" => "{} or",
        "Reset" => "Réinitialiser",
        "Take back every point spent, for free." => "Reprendre tous les points, gratuitement.",
        "Click to learn." => "Cliquez pour apprendre.",
        "Rank {}/{}" => "Rang {}/{}",
        "+{} per rank" => "+{} par rang",
        "The talent tables load with the game's data." => {
            "Les tables de talents se chargent avec les données du jeu."
        }
        "Horde" => "Horde",
        "Bosses" => "Boss",
        "Spells" => "Sorts",
        "Items" => "Objets",
        "Not played yet" => "Pas encore joué",
        "Take it from a level-up card." => "Prenez-le sur une carte de niveau.",
        "Claim" => "Réclamer",
        "Claim all" => "Tout réclamer",
        "Claimed" => "Réclamé",
        "Nothing waits to be claimed. Achievements pay gold here once done." => {
            "Rien à réclamer. Les hauts faits paient de l'or ici une fois accomplis."
        }
        "{}: always" => "{} : toujours",
        "Horde: {}" => "Horde : {}",
        "1 in {}" => "1 sur {}",
        "never" => "jamais",
        // Options.
        "Master volume" => "Volume général",
        "Music" => "Musique",
        "Sound effects" => "Effets sonores",
        "Mouse sensitivity" => "Sensibilité de la souris",
        "Picture polish" => "Finition de l'image",
        "On" => "Activé",
        "Off" => "Désactivé",
        "Language" => "Langue",
        "Controls" => "Commandes",
        "Run where the camera looks" => "Courir là où regarde la caméra",
        "Run forward" => "Courir droit devant",
        "Look around" => "Regarder autour",
        "Jump" => "Sauter",
        "Movement spell" => "Sort de déplacement",
        "Switch the camera" => "Changer de caméra",
        "Toggle the picture polish" => "Basculer la finition de l'image",
        "Pick a card" => "Choisir une carte",
        "Reroll the cards" => "Relancer les cartes",
        "Pause" => "Pause",
        "Both mouse buttons" => "Les deux boutons de la souris",
        // Stats, as bonuses read.
        "damage" => "dégâts",
        "max health" => "vie maximale",
        "movement speed" => "vitesse de déplacement",
        "health per second" => "vie par seconde",
        "cooldown recovery" => "récupération des temps de recharge",
        "area of effect" => "zone d'effet",
        "pickup range" => "rayon de ramassage",
        "experience" => "expérience",
        "damage blocked per hit" => "dégâts bloqués par coup",
        "critical strike" => "coup critique",
        "damage reduction" => "réduction des dégâts",
        "gold" => "or",
        // Slots.
        "Head" => "Tête",
        "Shoulders" => "Épaules",
        "Chest" => "Torse",
        "Wrists" => "Poignets",
        "Hands" => "Mains",
        "Waist" => "Taille",
        "Legs" => "Jambes",
        "Feet" => "Pieds",
        "Main Hand" => "Main droite",
        "Off Hand" => "Main gauche",
        // Rarities and difficulties.
        "Uncommon" => "Inhabituel",
        "Rare" => "Rare",
        "Epic" => "Épique",
        "Legendary" => "Légendaire",
        "Normal" => "Normal",
        "Heroic" => "Héroïque",
        "Mythic" => "Mythique",
        // Classes and races, as the French client names them.
        "Warrior" => "Guerrier",
        "Paladin" => "Paladin",
        "Hunter" => "Chasseur",
        "Rogue" => "Voleur",
        "Priest" => "Prêtre",
        "Shaman" => "Chaman",
        "Mage" => "Mage",
        "Warlock" => "Démoniste",
        "Druid" => "Druide",
        "Human" => "Humain",
        "Orc" => "Orc",
        "Dwarf" => "Nain",
        "Night Elf" => "Elfe de la nuit",
        "Undead" => "Mort-vivant",
        "Tauren" => "Tauren",
        "Gnome" => "Gnome",
        "Troll" => "Troll",
        // The campaign's zones.
        "Elwynn Forest" => "Forêt d'Elwynn",
        "Duskwood" => "Bois de la Pénombre",
        "Warsong Gulch" => "Goulet des Chanteguerres",
        "Un'Goro Crater" => "Cratère d'Un'Goro",
        "Arathi Basin" => "Bassin Arathi",
        "Feralas" => "Féralas",
        "Gurubashi Arena" => "Arène des Gurubashi",
        "Scarlet Monastery" => "Monastère Écarlate",
        "Alterac Valley" => "Vallée d'Alterac",
        "Blackrock Mountain" => "Mont Rochenoire",
        "Gates of Ahn'Qiraj" => "Portes d'Ahn'Qiraj",
        "Naxxramas" => "Naxxramas",
        // The run.
        "LEVEL UP" => "NIVEAU SUPÉRIEUR",
        "Choose your power" => "Choisissez votre pouvoir",
        "Click a card, or press 1, 2 or 3" => "Cliquez une carte, ou appuyez sur 1, 2 ou 3",
        "Click a card, or press 1, 2 or 3  ·  R rerolls the cards ({} left)" => {
            "Cliquez une carte, ou appuyez sur 1, 2 ou 3  ·  R relance les cartes ({} restantes)"
        }
        "Click a card, or press 1, 2 or 3  ·  no rerolls left" => {
            "Cliquez une carte, ou appuyez sur 1, 2 ou 3  ·  plus de relance"
        }
        "Victory!" => "Victoire !",
        "You have fallen" => "Vous êtes tombé",
        "The horde was too much this time." => "La horde était trop forte cette fois.",
        "Found: {}" => "Trouvé : {}",
        "Gold earned: {}" => "Or gagné : {}",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_as_written_and_french_is_looked_up() {
        // The global language is left alone: other tests read English through it.
        assert_eq!(tr_in(Lang::En, "Equipment"), "Equipment");
        assert_eq!(
            trf_in(Lang::En, "Enter {}", &["Duskwood"]),
            "Enter Duskwood"
        );
        assert_eq!(tr_in(Lang::Fr, "Equipment"), "Équipement");
        assert_eq!(
            trf_in(Lang::Fr, "Enter {}", &[tr_in(Lang::Fr, "Duskwood")]),
            "Entrer : Bois de la Pénombre"
        );
        assert_eq!(trf_in(Lang::Fr, "{} of {}", &["3", "12"]), "3 sur 12");
        // A text the table lacks stays as written.
        assert_eq!(tr_in(Lang::Fr, "Hogger"), "Hogger");
        // A refresh that has seen the generation draws once, then waits for the next.
        let mut seen = u32::MAX;
        assert!(stale(&mut seen));
        assert!(!stale(&mut seen));
        assert_eq!(Lang::from_code("FR"), Lang::Fr);
        assert_eq!(Lang::from_code("de"), Lang::En);
    }

    #[test]
    fn every_template_keeps_its_holes() {
        for (en, fr) in [
            ("Enter {}", "Entrer : {}"),
            ("Defeat {} at {} · {} bosses on the way", ""),
            ("Rank {}/{}", ""),
        ] {
            let got = french(en).unwrap();
            assert_eq!(en.matches("{}").count(), got.matches("{}").count(), "{en}");
            if !fr.is_empty() {
                assert_eq!(got, fr);
            }
        }
    }

    #[test]
    fn the_classes_zones_and_slots_have_their_french() {
        for c in super::super::data::CLASSES {
            assert!(french(c.name).is_some(), "{}", c.name);
        }
        for m in super::super::data::MAPS {
            assert!(french(m.name).is_some(), "{}", m.name);
        }
        for s in super::super::data::GearSlot::ALL {
            assert!(french(s.name()).is_some(), "{}", s.name());
        }
        for d in super::super::data::DIFFICULTIES {
            assert!(french(d.name).is_some(), "{}", d.name);
        }
    }
}
