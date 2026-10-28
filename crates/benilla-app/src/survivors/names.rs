//! Hero names: a random one in the race's own sound, and what a typed one may hold.
//!
//! Each race draws a start, sometimes a middle, and an end from its own syllables, so a Troll
//! comes out as "Zul'jin"-ish and a Night Elf as "Shandris"-ish. The syllables are the mode's,
//! written in the races' flavour; no client data is involved.

/// The longest a name may be, in letters.
pub(super) const MAX_NAME: usize = 16;

struct Syllables {
    start: &'static [&'static str],
    middle: &'static [&'static str],
    end: &'static [&'static str],
}

fn syllables(race: u8) -> &'static Syllables {
    const HUMAN: Syllables = Syllables {
        start: &[
            "Al", "Bran", "Cor", "Dar", "Ed", "Gar", "Hal", "Jor", "Mar", "Ro", "Tal", "Wil",
        ],
        middle: &["", "", "an", "ed", "el", "or"],
        end: &[
            "ric", "win", "den", "ford", "mond", "ton", "ard", "lan", "ius", "eth",
        ],
    };
    const ORC: Syllables = Syllables {
        start: &[
            "Gro", "Thra", "Dur", "Kar", "Naz", "Mok", "Gar", "Zag", "Brok", "Rok", "Ug", "Kro",
        ],
        middle: &["", "", "g", "ka", "ro", "th"],
        end: &[
            "mash", "gor", "rok", "nak", "tar", "gash", "dum", "zug", "lok", "gul",
        ],
    };
    const DWARF: Syllables = Syllables {
        start: &[
            "Thor", "Brom", "Dur", "Gim", "Mag", "Bel", "Ker", "Thal", "Hur", "Mor", "Grim", "Bal",
        ],
        middle: &["", "", "da", "ri", "un", "ga"],
        end: &[
            "din", "nar", "grim", "li", "rak", "bek", "dun", "gar", "mok", "ric",
        ],
    };
    const NIGHT_ELF: Syllables = Syllables {
        start: &[
            "Shan", "Ty", "Fan", "Ila", "Ara", "Mal", "Ely", "Sye", "Lor", "Ner", "Ash", "Vel",
        ],
        middle: &["", "", "dra", "ri", "the", "lan"],
        end: &[
            "dris", "rande", "dral", "nar", "thas", "wyn", "riel", "dor", "las", "ael",
        ],
    };
    const UNDEAD: Syllables = Syllables {
        start: &[
            "Mor", "Ver", "Sal", "Ghor", "Nath", "Var", "Kel", "Vex", "Sor", "Dra", "Lich", "Gris",
        ],
        middle: &["", "", "e", "o", "ul", "is"],
        end: &[
            "ion", "rath", "mire", "goth", "ius", "drek", "thul", "vane", "rix", "mus",
        ],
    };
    const TAUREN: Syllables = Syllables {
        start: &[
            "Ahn", "Bai", "Ha", "Tah", "Mu", "Ko", "Ta", "Hu", "Ma", "Ruu", "Ne", "Ohan",
        ],
        middle: &["", "", "wa", "ho", "ka", "lu"],
        end: &[
            "hoof", "mane", "horn", "totem", "runner", "storm", "wind", "tusk", "hide", "son",
        ],
    };
    const GNOME: Syllables = Syllables {
        start: &[
            "Fiz", "Gel", "Mek", "Tink", "Spro", "Bink", "Wiz", "Nod", "Pip", "Glim", "Tob", "Zap",
        ],
        middle: &["", "", "zle", "bo", "ni", "le"],
        end: &[
            "bolt", "wick", "sprock", "gear", "nozzle", "fizz", "spark", "tink", "wobble", "pin",
        ],
    };
    const TROLL: Syllables = Syllables {
        start: &[
            "Zul", "Vol", "Jin", "Rokh", "Sen", "Zal", "Kaz", "Ra", "Nir", "Hak", "Vo", "Tal",
        ],
        middle: &["'", "", "'", "ka", "ja", "sha"],
        end: &[
            "jin", "zan", "dar", "kah", "jai", "rak", "zo", "mah", "shed", "tik",
        ],
    };
    const BLOOD_ELF: Syllables = Syllables {
        start: &[
            "Kael", "Lor", "Sil", "Ael", "Val", "Ly", "Hal", "Rom", "Fael", "Quel", "Ana", "Ely",
        ],
        middle: &["", "", "thas", "ia", "an", "dor"],
        end: &[
            "thas", "lia", "vanna", "duin", "orin", "dris", "riel", "athor", "tharas", "nei",
        ],
    };
    const DRAENEI: Syllables = Syllables {
        start: &[
            "Vel", "Nob", "Mar", "Iri", "Ama", "Dur", "Kur", "Nah", "Ved", "Zor", "Ish", "Tor",
        ],
        middle: &["", "", "a", "u", "aa", "e"],
        end: &[
            "en", "undu", "aad", "el", "uun", "osh", "aan", "ari", "um", "ek",
        ],
    };
    const GOBLIN: Syllables = Syllables {
        start: &[
            "Gaz", "Riz", "Noz", "Grem", "Kez", "Bix", "Zap", "Fizz", "Glee", "Trix", "Sprock",
            "Wiz",
        ],
        middle: &["", "", "le", "z", "ik", "o"],
        end: &[
            "lowe", "zle", "dle", "bolt", "gear", "nix", "sprock", "wick", "spark", "tock",
        ],
    };
    const WORGEN: Syllables = Syllables {
        start: &[
            "Gen", "Dar", "Iv", "Liam", "Lor", "Hal", "Cro", "God", "Bel", "Ash", "Wal", "Tess",
        ],
        middle: &["", "", "a", "e", "wyn", "fe"],
        end: &[
            "ius", "ford", "ric", "na", "wood", "ley", "gate", "more", "ard", "wen",
        ],
    };
    const PANDAREN: Syllables = Syllables {
        start: &[
            "Chen", "Li", "Mei", "Lo", "Tai", "Shu", "Jin", "Ken", "Mu", "Ping", "Yu", "Hao",
        ],
        middle: &["", "", "", "an", "o", "li"],
        end: &[
            "", "lan", "shi", "po", "ming", "fu", "zhi", "tao", "ning", "wei",
        ],
    };
    match race {
        24 => &PANDAREN,
        9 => &GOBLIN,
        22 => &WORGEN,
        10 => &BLOOD_ELF,
        11 => &DRAENEI,
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

/// A random name in the race's sound, from `roll` (uniform in `[0, 1)`).
pub(super) fn random_name(race: u8, mut roll: impl FnMut() -> f32) -> String {
    let s = syllables(race);
    let mut pick =
        |list: &[&'static str]| list[((roll() * list.len() as f32) as usize).min(list.len() - 1)];
    let start = pick(s.start);
    let middle = pick(s.middle);
    let end = pick(s.end);
    // An apostrophe never ends a start that already ends in one, nor doubles up.
    let middle = if middle == "'" && (start.ends_with('\'') || end.starts_with('\'')) {
        ""
    } else {
        middle
    };
    let name = clean_name(&format!("{start}{middle}{end}"));
    if name.chars().count() >= 3 {
        name
    } else {
        "Adventurer".to_string()
    }
}

/// A typed name kept to what a name may hold: letters and an apostrophe inside it (not two in a
/// row, not at the ends), the first letter capitalised, the rest as typed, at most [`MAX_NAME`].
pub(super) fn clean_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if out.chars().count() >= MAX_NAME {
            break;
        }
        if c.is_alphabetic() {
            out.push(c);
        } else if c == '\'' && !out.is_empty() && !out.ends_with('\'') {
            out.push(c);
        }
    }
    while out.ends_with('\'') {
        out.pop();
    }
    let mut chars = out.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => out,
    }
}

/// Whether a character may be typed into a name.
pub(super) fn name_char(c: char) -> bool {
    c.is_alphabetic() || c == '\''
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(values: &[f32]) -> impl FnMut() -> f32 + '_ {
        let mut i = 0;
        move || {
            let v = values[i % values.len()];
            i += 1;
            v
        }
    }

    #[test]
    fn every_race_makes_clean_names() {
        for race in 1..=8u8 {
            for k in 0..50 {
                let f = (k as f32) / 50.0;
                let name = random_name(race, seq(&[f, 1.0 - f, (f * 7.3).fract()]));
                assert!(name.chars().count() >= 3, "{race}: {name}");
                assert!(name.chars().count() <= MAX_NAME, "{race}: {name}");
                assert_eq!(clean_name(&name), name, "{race}: {name} is already clean");
                assert!(name.chars().next().unwrap().is_uppercase(), "{name}");
            }
        }
    }

    #[test]
    fn races_sound_different() {
        let troll = random_name(8, seq(&[0.0, 0.0, 0.0]));
        let gnome = random_name(7, seq(&[0.0, 0.0, 0.0]));
        assert_ne!(troll, gnome);
        assert!(troll.starts_with("Zul"));
    }

    #[test]
    fn typed_names_are_kept_tidy() {
        assert_eq!(clean_name("nirkashed"), "Nirkashed");
        assert_eq!(clean_name("  zul''jin42!"), "Zul'jin");
        assert_eq!(clean_name("'thrall'"), "Thrall");
        assert_eq!(clean_name("élise"), "Élise");
        assert_eq!(clean_name(&"a".repeat(40)).chars().count(), MAX_NAME);
        assert_eq!(clean_name("123"), "");
        assert!(name_char('x') && name_char('\'') && !name_char('1') && !name_char(' '));
    }
}
