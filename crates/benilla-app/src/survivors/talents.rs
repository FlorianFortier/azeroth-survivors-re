//! Talents: the class's three real trees from `Talent.dbc` and `TalentTab.dbc` (their names,
//! icons, grid seats, ranks and prerequisites, under vanilla's rules: five points in a tree per
//! tier, a prerequisite maxed first, 51 points at most), with the mode's own numbers. Each tree
//! has a theme of three stats after what it does in WoW (Arms hits harder and crits, Protection
//! takes less), and each talent gives one of them per rank, an exceptional talent (the tree's big
//! abilities) three times as much.
//!
//! Points are trained with the gold the runs earn ([`point_cost`]); they are the account's, and
//! each class spends them on its own trees.

use std::collections::BTreeMap;

use benilla_formats::{Talent, TalentCatalog, TalentTabInfo};

use super::data::{Bonus, Stat};

/// Vanilla's 51 points at level 60.
pub(super) const MAX_POINTS: u32 = 51;
/// Points needed in a tree per tier.
pub(super) const POINTS_PER_TIER: u32 = 5;

/// The gold the next point costs, `bought` points in.
pub(super) fn point_cost(bought: u32) -> u64 {
    20 + 10 * u64::from(bought)
}

/// A class's spending: talent id (as text, for the profile file) to its rank.
pub(super) type Spent = BTreeMap<String, u8>;

/// One tree: its tab and its talents.
#[derive(Clone, Copy)]
pub(super) struct Tree<'a> {
    pub tab: &'a TalentTabInfo,
    pub talents: &'a [Talent],
}

/// A race and class's trees, in the client's order.
pub(super) fn trees(catalog: &TalentCatalog, race: u8, class: u8) -> Vec<Tree<'_>> {
    catalog
        .tabs_for_class(race.max(1), class)
        .into_iter()
        .map(|tab| Tree {
            tab,
            talents: catalog.talents_in_tab(tab.id),
        })
        .collect()
}

/// A tree's three stats, after what it does in WoW; by its art name ("MageFire").
pub(super) fn theme(background: &str) -> [Stat; 3] {
    use Stat::*;
    match background.to_ascii_lowercase().as_str() {
        "warriorarms" => [Might, Crit, Armor],
        "warriorfury" => [Haste, Might, Crit],
        "warriorprotection" => [MaxHp, Armor, DamageReduction],
        "paladinholy" => [Regen, MaxHp, Growth],
        "paladinprotection" => [Armor, DamageReduction, MaxHp],
        "paladincombat" => [Might, Crit, Haste],
        "hunterbeastmastery" => [Haste, Speed, Might],
        "huntermarksmanship" => [Crit, Might, Area],
        "huntersurvival" => [Speed, DamageReduction, Magnet],
        "rogueassassination" => [Crit, Might, Haste],
        "roguecombat" => [Haste, Might, Speed],
        "roguesubtlety" => [DamageReduction, Speed, Crit],
        "priestdiscipline" => [Haste, MaxHp, Growth],
        "priestholy" => [Regen, MaxHp, Area],
        "priestshadow" => [Might, Area, Crit],
        "shamanelementalcombat" => [Might, Area, Crit],
        "shamanenhancement" => [Haste, Might, Armor],
        "shamanrestoration" => [Regen, MaxHp, Haste],
        "magearcane" => [Haste, Growth, Area],
        "magefire" => [Might, Crit, Area],
        "magefrost" => [DamageReduction, Area, Crit],
        "warlockcurses" => [Might, Area, Regen],
        "warlocksummoning" => [MaxHp, Armor, Regen],
        "warlockdestruction" => [Might, Crit, Haste],
        "druidbalance" => [Area, Might, Crit],
        "druidferalcombat" => [Armor, Speed, Crit],
        "druidrestoration" => [Regen, MaxHp, Magnet],
        _ => [Might, MaxHp, Crit],
    }
}

/// What one rank of a talent on `stat` gives.
fn per_rank(stat: Stat) -> Bonus {
    match stat {
        Stat::Might => Bonus::flat(Stat::Might, 0.01),
        Stat::MaxHp => Bonus::pct(Stat::MaxHp, 0.01),
        Stat::Speed => Bonus::pct(Stat::Speed, 0.01),
        Stat::Regen => Bonus::flat(Stat::Regen, 0.2),
        Stat::Haste => Bonus::flat(Stat::Haste, 0.01),
        Stat::Area => Bonus::flat(Stat::Area, 0.01),
        Stat::Magnet => Bonus::pct(Stat::Magnet, 0.02),
        Stat::Growth => Bonus::flat(Stat::Growth, 0.01),
        Stat::Armor => Bonus::flat(Stat::Armor, 0.5),
        Stat::Crit => Bonus::flat(Stat::Crit, 0.005),
        Stat::DamageReduction => Bonus::flat(Stat::DamageReduction, 0.005),
        Stat::Gold => Bonus::pct(Stat::Gold, 0.02),
    }
}

/// What one rank of `talent` in `tab` gives: its seat picks one of the tree's three stats.
pub(super) fn rank_bonus(tab: &TalentTabInfo, talent: &Talent) -> Bonus {
    let stats = theme(&tab.background);
    let stat = stats[((talent.row * 4 + talent.col) % 3) as usize];
    let b = per_rank(stat);
    if talent.exceptional {
        scale(b, 3.0)
    } else {
        b
    }
}

fn scale(b: Bonus, k: f32) -> Bonus {
    use super::data::Amount;
    Bonus {
        stat: b.stat,
        amount: match b.amount {
            Amount::Flat(v) => Amount::Flat(v * k),
            Amount::Percent(v) => Amount::Percent(v * k),
        },
    }
}

/// A talent's rank in `spent`.
pub(super) fn rank(spent: &Spent, talent: &Talent) -> u8 {
    spent.get(&talent.id.to_string()).copied().unwrap_or(0)
}

/// Points spent in one tree.
pub(super) fn spent_in(spent: &Spent, tree: &Tree) -> u32 {
    tree.talents.iter().map(|t| u32::from(rank(spent, t))).sum()
}

/// Points spent in all the class's trees.
pub(super) fn spent_total(spent: &Spent, trees: &[Tree]) -> u32 {
    trees.iter().map(|t| spent_in(spent, t)).sum()
}

/// Why a rank cannot be learned now, or `Ok` when it can: points left, the tier opened by five
/// points a tier in this tree, the prerequisite maxed, and a rank left.
pub(super) fn can_learn(
    spent: &Spent,
    trees: &[Tree],
    tree: &Tree,
    talent: &Talent,
    points: u32,
) -> Result<(), String> {
    let total = spent_total(spent, trees);
    if total >= points.min(MAX_POINTS) {
        return Err(if points < MAX_POINTS {
            "No talent points left: train another.".into()
        } else {
            "Every point is spent.".into()
        });
    }
    if u32::from(rank(spent, talent)) >= talent.max_rank() {
        return Err("Every rank is learned.".into());
    }
    let need = talent.row * POINTS_PER_TIER;
    if spent_in(spent, tree) < need {
        return Err(format!("Requires {need} points in {}.", tree.tab.name));
    }
    if talent.prereq_talent != 0 {
        if let Some(pre) = tree.talents.iter().find(|t| t.id == talent.prereq_talent) {
            if u32::from(rank(spent, pre)) < talent.prereq_rank + 1 {
                return Err("Requires the talent before it.".into());
            }
        }
    }
    Ok(())
}

/// Learn a rank, if it can be.
pub(super) fn learn(
    spent: &mut Spent,
    trees: &[Tree],
    tree: &Tree,
    talent: &Talent,
    points: u32,
) -> bool {
    if can_learn(spent, trees, tree, talent, points).is_err() {
        return false;
    }
    *spent.entry(talent.id.to_string()).or_default() += 1;
    true
}

/// What the class's talents bring into a run.
pub(super) fn bonuses(spent: &Spent, trees: &[Tree]) -> Vec<Bonus> {
    let mut out = Vec::new();
    for tree in trees {
        for t in tree.talents {
            let r = rank(spent, t);
            if r > 0 {
                out.push(scale(rank_bonus(tree.tab, t), f32::from(r)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: u32, background: &str) -> TalentTabInfo {
        TalentTabInfo {
            id,
            name: background.into(),
            race_mask: 511,
            class_mask: 1,
            background: background.into(),
        }
    }

    fn talent(id: u32, row: u32, col: u32, ranks: u32) -> Talent {
        let mut r = [0; benilla_formats::MAX_TALENT_RANK];
        for (i, s) in r.iter_mut().enumerate().take(ranks as usize) {
            *s = 1000 + id * 10 + i as u32;
        }
        Talent {
            id,
            tab: 1,
            row,
            col,
            ranks: r,
            prereq_talent: 0,
            prereq_rank: 0,
            required_spell: 0,
            exceptional: false,
        }
    }

    #[test]
    fn tiers_open_five_points_at_a_time_and_prerequisites_come_first() {
        let arms = tab(1, "WarriorArms");
        let mut second = talent(2, 1, 0, 3);
        second.prereq_talent = 1;
        second.prereq_rank = 4;
        let list = vec![talent(1, 0, 0, 5), second, talent(3, 1, 1, 2)];
        let tree = Tree {
            tab: &arms,
            talents: &list,
        };
        let trees = [tree];
        let mut spent = Spent::new();
        // Tier 2 is shut until five points are in the tree.
        assert!(can_learn(&spent, &trees, &tree, &list[2], 51).is_err());
        for _ in 0..4 {
            assert!(learn(&mut spent, &trees, &tree, &list[0], 51));
        }
        assert!(can_learn(&spent, &trees, &tree, &list[2], 51).is_err());
        assert!(learn(&mut spent, &trees, &tree, &list[0], 51));
        // A sixth rank of a five-rank talent: no.
        assert!(!learn(&mut spent, &trees, &tree, &list[0], 51));
        // Tier 2 now open, and the prerequisite (rank 5) is met.
        assert!(learn(&mut spent, &trees, &tree, &list[1], 51));
        assert!(learn(&mut spent, &trees, &tree, &list[2], 51));
        assert_eq!(spent_total(&spent, &trees), 7);
        // Out of points.
        assert!(!learn(&mut spent, &trees, &tree, &list[2], 7));
    }

    #[test]
    fn a_prerequisite_not_maxed_blocks() {
        let fire = tab(1, "MageFire");
        let mut needs = talent(2, 0, 1, 1);
        needs.prereq_talent = 1;
        needs.prereq_rank = 2;
        let list = vec![talent(1, 0, 0, 3), needs];
        let tree = Tree {
            tab: &fire,
            talents: &list,
        };
        let trees = [tree];
        let mut spent = Spent::new();
        learn(&mut spent, &trees, &tree, &list[0], 51);
        learn(&mut spent, &trees, &tree, &list[0], 51);
        assert!(can_learn(&spent, &trees, &tree, &list[1], 51).is_err());
        learn(&mut spent, &trees, &tree, &list[0], 51);
        assert!(can_learn(&spent, &trees, &tree, &list[1], 51).is_ok());
    }

    #[test]
    fn ranks_add_up_on_the_trees_theme() {
        let prot = tab(1, "WarriorProtection");
        let mut big = talent(2, 0, 1, 1);
        big.exceptional = true;
        let list = vec![talent(1, 0, 0, 5), big];
        let tree = Tree {
            tab: &prot,
            talents: &list,
        };
        let trees = [tree];
        let mut spent = Spent::new();
        for _ in 0..3 {
            learn(&mut spent, &trees, &tree, &list[0], 51);
        }
        learn(&mut spent, &trees, &tree, &list[1], 51);
        let b = bonuses(&spent, &trees);
        assert_eq!(b.len(), 2);
        let theme_stats = theme("WarriorProtection");
        assert!(b.iter().all(|x| theme_stats.contains(&x.stat)));
        // Three ranks of 1% health.
        assert_eq!(b[0], Bonus::pct(Stat::MaxHp, 0.03));
        // An exceptional talent's one rank is worth three.
        assert_eq!(b[1], scale(per_rank(theme_stats[1]), 3.0));
    }

    #[test]
    fn every_vanilla_tree_has_a_theme_and_points_cost_more_each() {
        for bg in [
            "WarriorArms",
            "WarriorFury",
            "WarriorProtection",
            "PaladinHoly",
            "PaladinProtection",
            "PaladinCombat",
            "HunterBeastMastery",
            "HunterMarksmanship",
            "HunterSurvival",
            "RogueAssassination",
            "RogueCombat",
            "RogueSubtlety",
            "PriestDiscipline",
            "PriestHoly",
            "PriestShadow",
            "ShamanElementalCombat",
            "ShamanEnhancement",
            "ShamanRestoration",
            "MageArcane",
            "MageFire",
            "MageFrost",
            "WarlockCurses",
            "WarlockSummoning",
            "WarlockDestruction",
            "DruidBalance",
            "DruidFeralCombat",
            "DruidRestoration",
        ] {
            let t = theme(bg);
            assert_ne!(t, theme("unknown"), "{bg} falls to the default");
        }
        assert!(point_cost(1) > point_cost(0));
        let all: u64 = (0..MAX_POINTS).map(point_cost).sum();
        assert_eq!(all, 20 * 51 + 10 * (50 * 51 / 2));
    }
}
