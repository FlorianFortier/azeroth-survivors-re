//! What a lobby's host can tune, and the bounds every value is held to. The server clamps what a
//! host sends, and the game clamps again before it plays: neither trusts the other's numbers.

use crate::wire::{Reader, Wire, WireError, Writer};

/// The most players a lobby can seat.
pub const MAX_PLAYERS: u8 = 4;
/// The zones the game offers (`survivors::data::MAPS`); the game's tests hold the two equal.
pub const ZONE_COUNT: u8 = 12;
/// Longest lobby name, in characters.
pub const MAX_LOBBY_NAME: usize = 32;
/// Longest player name, in characters.
pub const MAX_PLAYER_NAME: usize = 20;

/// When the final boss comes; the other bosses' times scale with it.
pub const DURATION_MIN: u16 = 5 * 60;
pub const DURATION_MAX: u16 = 30 * 60;
pub const DURATION_DEFAULT: u16 = 16 * 60;
/// How often the horde closes a ring around every hero.
pub const AMBUSH_EVERY_MIN: u16 = 60;
pub const AMBUSH_EVERY_MAX: u16 = 300;
pub const AMBUSH_EVERY_DEFAULT: u16 = 135;
/// Percent bounds for the three difficulty multipliers.
pub const PERCENT_MIN: u16 = 25;
pub const PERCENT_MAX: u16 = 400;

/// The enemies' strength, in percent of the solo tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    pub enemy_health: u16,
    pub enemy_damage: u16,
    /// How many enemies are kept alive at once.
    pub density: u16,
    /// Health and density grow with the number of heroes.
    pub scale_with_players: bool,
}

impl Default for Difficulty {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl Difficulty {
    pub const EASY: Self = Self {
        enemy_health: 70,
        enemy_damage: 60,
        density: 80,
        scale_with_players: true,
    };
    pub const NORMAL: Self = Self {
        enemy_health: 100,
        enemy_damage: 100,
        density: 100,
        scale_with_players: true,
    };
    pub const HARD: Self = Self {
        enemy_health: 150,
        enemy_damage: 140,
        density: 125,
        scale_with_players: true,
    };
    pub const NIGHTMARE: Self = Self {
        enemy_health: 220,
        enemy_damage: 200,
        density: 150,
        scale_with_players: true,
    };

    /// The named preset these numbers match, if any.
    pub fn preset_name(&self) -> &'static str {
        let mut plain = *self;
        plain.scale_with_players = true;
        match plain {
            p if p == Self::EASY => "Easy",
            p if p == Self::NORMAL => "Normal",
            p if p == Self::HARD => "Hard",
            p if p == Self::NIGHTMARE => "Nightmare",
            _ => "Custom",
        }
    }
}

/// A lobby's rules for the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub name: String,
    /// An index into the game's zones.
    pub zone: u8,
    pub max_players: u8,
    pub difficulty: Difficulty,
    /// When the final boss comes; its fall wins the run.
    pub duration_secs: u16,
    /// How often the horde closes a ring around every hero.
    pub ambush_every_secs: u16,
    /// Every hero gains the experience any hero picks up.
    pub shared_xp: bool,
    /// A fallen hero can be brought back by an ally standing over them.
    pub revive: bool,
    /// Shown in the server's lobby list; a private lobby is joined by code only.
    pub listed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: String::from("Azeroth Survivors"),
            zone: 0,
            max_players: MAX_PLAYERS,
            difficulty: Difficulty::NORMAL,
            duration_secs: DURATION_DEFAULT,
            ambush_every_secs: AMBUSH_EVERY_DEFAULT,
            shared_xp: true,
            revive: true,
            listed: false,
        }
    }
}

/// Keep printable characters, collapse runs of spaces, trim, and cut to `max` characters.
pub fn clean_name(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
        if out.chars().count() >= max {
            break;
        }
    }
    out
}

impl Settings {
    /// The same settings held inside every bound.
    pub fn sanitized(&self) -> Self {
        let pct = |v: u16| v.clamp(PERCENT_MIN, PERCENT_MAX);
        let name = clean_name(&self.name, MAX_LOBBY_NAME);
        Self {
            name: if name.is_empty() {
                Settings::default().name
            } else {
                name
            },
            zone: self.zone.min(ZONE_COUNT - 1),
            max_players: self.max_players.clamp(1, MAX_PLAYERS),
            difficulty: Difficulty {
                enemy_health: pct(self.difficulty.enemy_health),
                enemy_damage: pct(self.difficulty.enemy_damage),
                density: pct(self.difficulty.density),
                scale_with_players: self.difficulty.scale_with_players,
            },
            duration_secs: self.duration_secs.clamp(DURATION_MIN, DURATION_MAX),
            ambush_every_secs: self
                .ambush_every_secs
                .clamp(AMBUSH_EVERY_MIN, AMBUSH_EVERY_MAX),
            shared_xp: self.shared_xp,
            revive: self.revive,
            listed: self.listed,
        }
    }
}

impl Wire for Difficulty {
    fn put(&self, w: &mut Writer) {
        w.u16(self.enemy_health)
            .u16(self.enemy_damage)
            .u16(self.density)
            .bool(self.scale_with_players);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            enemy_health: r.u16()?,
            enemy_damage: r.u16()?,
            density: r.u16()?,
            scale_with_players: r.bool()?,
        })
    }
}

impl Wire for Settings {
    fn put(&self, w: &mut Writer) {
        w.str(&self.name).u8(self.zone).u8(self.max_players);
        self.difficulty.put(w);
        w.u16(self.duration_secs)
            .u16(self.ambush_every_secs)
            .bool(self.shared_xp)
            .bool(self.revive)
            .bool(self.listed);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.str()?,
            zone: r.u8()?,
            max_players: r.u8()?,
            difficulty: Difficulty::get(r)?,
            duration_secs: r.u16()?,
            ambush_every_secs: r.u16()?,
            shared_xp: r.bool()?,
            revive: r.bool()?,
            listed: r.bool()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_already_sane() {
        let s = Settings::default();
        assert_eq!(s.sanitized(), s);
    }

    #[test]
    fn sanitize_clamps_every_bound() {
        let wild = Settings {
            name: "  \u{7}  ".into(),
            zone: 200,
            max_players: 0,
            difficulty: Difficulty {
                enemy_health: 0,
                enemy_damage: 9999,
                density: 1,
                scale_with_players: false,
            },
            duration_secs: 1,
            ambush_every_secs: u16::MAX,
            shared_xp: false,
            revive: false,
            listed: true,
        };
        let s = wild.sanitized();
        assert_eq!(s.name, Settings::default().name);
        assert_eq!(s.zone, ZONE_COUNT - 1);
        assert_eq!(s.max_players, 1);
        assert_eq!(s.difficulty.enemy_health, PERCENT_MIN);
        assert_eq!(s.difficulty.enemy_damage, PERCENT_MAX);
        assert_eq!(s.difficulty.density, PERCENT_MIN);
        assert_eq!(s.duration_secs, DURATION_MIN);
        assert_eq!(s.ambush_every_secs, AMBUSH_EVERY_MAX);
        assert!(s.listed && !s.revive && !s.shared_xp);
        let many = Settings {
            max_players: 99,
            ..Settings::default()
        };
        assert_eq!(many.sanitized().max_players, MAX_PLAYERS);
    }

    #[test]
    fn names_are_cleaned_and_cut() {
        assert_eq!(
            clean_name("  Jaina \n\t Proudmoore ", 20),
            "Jaina Proudmoore"
        );
        assert_eq!(clean_name("a\u{0}b", 20), "ab");
        assert_eq!(clean_name(&"x".repeat(100), 20).len(), 20);
    }

    #[test]
    fn presets_are_named() {
        assert_eq!(Difficulty::HARD.preset_name(), "Hard");
        let mut custom = Difficulty::NORMAL;
        custom.density = 101;
        assert_eq!(custom.preset_name(), "Custom");
    }

    #[test]
    fn settings_round_trip() {
        let s = Settings {
            name: "Raid night".into(),
            zone: 7,
            max_players: 3,
            difficulty: Difficulty::HARD,
            duration_secs: 1200,
            ambush_every_secs: 90,
            shared_xp: false,
            revive: true,
            listed: true,
        };
        assert_eq!(Settings::decode(&s.encode()).unwrap(), s);
    }
}
