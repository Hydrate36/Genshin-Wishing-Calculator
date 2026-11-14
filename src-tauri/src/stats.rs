//! Pity, soft/hard pity, 50/50 results, earlys and the odds for the next wishes.
//! Rates are the commonly used community model of the game's datamined rates:
//! character/standard/chronicled 0.6% per wish, +6% per wish from wish 74, certain at 90;
//! weapon 0.7%, +7% from wish 63, certain at 80. 4★: 5.1% (weapon 6%), sharply up from wish 9 (8), certain at 10.
//! Star Rail is the same except Light Cones: 0.8%, +7% from warp 66, certain at 80 (4★ 6.6%).
use crate::data::Wish;
use crate::game::Game;
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct Rates {
    pub base: f64,
    pub soft: u32,
    pub step: f64,
    pub hard: u32,
}

pub fn rates5(g: Game, pool: &str) -> Rates {
    match (g, pool) {
        (Game::Genshin, "weapon") => Rates { base: 0.007, soft: 63, step: 0.07, hard: 80 },
        (Game::Hsr, "weapon") => Rates { base: 0.008, soft: 66, step: 0.07, hard: 80 },
        _ => Rates { base: 0.006, soft: 74, step: 0.06, hard: 90 },
    }
}

pub fn rates4(g: Game, pool: &str) -> Rates {
    match (g, pool) {
        (Game::Genshin, "weapon") => Rates { base: 0.06, soft: 8, step: 0.6, hard: 10 },
        (Game::Hsr, "weapon") => Rates { base: 0.066, soft: 8, step: 0.6, hard: 10 },
        _ => Rates { base: 0.051, soft: 9, step: 0.51, hard: 10 },
    }
}

/// Chance that wish number `n` since the last 5★ is a 5★.
pub fn p_at(r: Rates, n: u32) -> f64 {
    if n >= r.hard {
        return 1.0;
    }
    if n < r.soft {
        r.base
    } else {
        (r.base + r.step * (n - r.soft + 1) as f64).min(1.0)
    }
}

/// Chance of at least one 5★ in the next `k` wishes when `pity` wishes have gone by.
pub fn chance_within(r: Rates, pity: u32, k: u32) -> f64 {
    let mut miss = 1.0;
    for i in 1..=k {
        miss *= 1.0 - p_at(r, pity + i);
        if miss <= 0.0 {
            return 1.0;
        }
    }
    1.0 - miss
}

/// Average wishes per 5★ under the model.
pub fn expected_pity(r: Rates) -> f64 {
    let (mut miss, mut e) = (1.0, 0.0);
    for n in 1..=r.hard {
        let p = p_at(r, n);
        e += n as f64 * miss * p;
        miss *= 1.0 - p;
    }
    e
}

/// Wishes still expected until the next 5★, from the current pity.
pub fn expected_left(r: Rates, pity: u32) -> f64 {
    let (mut miss, mut e) = (1.0, 0.0);
    for n in (pity + 1)..=r.hard {
        let p = p_at(r, n);
        e += (n - pity) as f64 * miss * p;
        miss *= 1.0 - p;
    }
    e
}

/// Chance a 5★ comes before soft pity ("early").
pub fn expected_early(r: Rates) -> f64 {
    1.0 - (1.0 - r.base).powi(r.soft as i32 - 1)
}

#[derive(Serialize, Clone, Debug)]
pub struct Five {
    pub name: String,
    pub id: String,
    pub icon: String,
    pub item_type: String,
    pub time: String,
    pub pity: u32,
    /// "won", "lost", "guaranteed", or "" where it doesn't apply
    pub result: String,
    pub early: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct Recent {
    pub name: String,
    pub id: String,
    pub icon: String,
    pub item_type: String,
    pub rank: u8,
    pub time: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct PoolStats {
    pub pool: String,
    pub total: u32,
    pub pity5: u32,
    pub pity4: u32,
    pub soft: u32,
    pub hard: u32,
    pub hard4: u32,
    /// "normal", "soft" (in soft pity) or "hard" (next wish is certain)
    pub zone: String,
    /// for character/weapon: the next 5★ is the featured one for sure
    pub guaranteed: Option<bool>,
    pub chance_next: f64,
    pub chance10: f64,
    pub expected_left: f64,
    pub fives: Vec<Five>,
    pub fours: u32,
    pub avg_pity: f64,
    pub expected_avg: f64,
    pub won: u32,
    pub lost: u32,
    pub early: u32,
    pub expected_early: f64,
    pub recent: Vec<Recent>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Summary {
    pub total: u32,
    pub primogems: u64,
    pub fives: u32,
    pub fours: u32,
    /// character banner 50/50s
    pub won: u32,
    pub lost: u32,
    /// weapon banner 75/25s
    pub weapon_won: u32,
    pub weapon_lost: u32,
    /// 5★ before soft pity, over every banner
    pub early: u32,
    pub expected_early: f64,
    pub avg_pity: f64,
    pub expected_avg: f64,
    pub pools: Vec<PoolStats>,
}

/// Looks up whether a 5★ was featured when pulled, and an item's picture.
pub trait Catalog {
    fn featured(&self, g: Game, pool: &str, w: &Wish, server: i64) -> Option<bool>;
    fn icon(&self, g: Game, w: &Wish) -> String;
}

pub fn pool_stats(g: Game, pool_name: &str, wishes: &[&Wish], cat: &dyn Catalog, server: i64) -> PoolStats {
    let r5 = rates5(g, pool_name);
    let r4 = rates4(g, pool_name);
    let has_fifty = matches!(pool_name, "character" | "weapon");
    let (mut c5, mut c4) = (0u32, 0u32);
    let mut fives: Vec<Five> = vec![];
    let mut fours = 0;
    let mut last_lost = false;
    for w in wishes {
        c5 += 1;
        c4 += 1;
        if w.rank == 5 {
            let mut result = String::new();
            if has_fifty {
                match cat.featured(g, pool_name, w, server) {
                    Some(true) => result = if last_lost { "guaranteed".into() } else { "won".into() },
                    Some(false) => result = "lost".into(),
                    None => {}
                }
                if !result.is_empty() {
                    last_lost = result == "lost";
                }
            }
            fives.push(Five {
                name: w.name.clone(),
                id: crate::banners::item_id(&w.name),
                icon: cat.icon(g, w),
                item_type: w.item_type.clone(),
                time: w.time.clone(),
                pity: c5,
                result,
                early: c5 < r5.soft,
            });
            c5 = 0;
        } else if w.rank == 4 {
            fours += 1;
            c4 = 0;
        }
    }
    let zone = if c5 + 1 >= r5.hard {
        "hard"
    } else if c5 + 1 >= r5.soft {
        "soft"
    } else {
        "normal"
    };
    let n = fives.len().max(1) as f64;
    let recent = wishes
        .iter()
        .rev()
        .filter(|w| w.rank >= 4)
        .take(12)
        .map(|w| Recent { name: w.name.clone(), id: crate::banners::item_id(&w.name), icon: cat.icon(g, w), item_type: w.item_type.clone(), rank: w.rank, time: w.time.clone() })
        .collect();
    PoolStats {
        pool: pool_name.into(),
        total: wishes.len() as u32,
        pity5: c5,
        pity4: c4,
        soft: r5.soft,
        hard: r5.hard,
        hard4: r4.hard,
        zone: zone.into(),
        guaranteed: has_fifty.then_some(last_lost),
        chance_next: p_at(r5, c5 + 1),
        chance10: chance_within(r5, c5, 10),
        expected_left: expected_left(r5, c5),
        avg_pity: if fives.is_empty() { 0.0 } else { fives.iter().map(|f| f.pity as f64).sum::<f64>() / n },
        expected_avg: expected_pity(r5),
        won: fives.iter().filter(|f| f.result == "won").count() as u32,
        lost: fives.iter().filter(|f| f.result == "lost").count() as u32,
        early: fives.iter().filter(|f| f.early).count() as u32,
        expected_early: expected_early(r5),
        fives,
        fours,
        recent,
    }
}

pub fn summary(g: Game, all: &[Wish], cat: &dyn Catalog, uid: &str) -> Summary {
    let server = crate::game::server_offset(uid);
    let pools: Vec<PoolStats> = g
        .pools()
        .iter()
        .map(|p| {
            let ws: Vec<&Wish> = all.iter().filter(|w| g.pool(&w.gacha_type) == *p).collect();
            pool_stats(g, p, &ws, cat, server)
        })
        .collect();
    let all_fives: Vec<(&PoolStats, &Five)> = pools.iter().filter(|p| p.pool != "beginner").flat_map(|p| p.fives.iter().map(move |f| (p, f))).collect();
    let nf = all_fives.len().max(1) as f64;
    let ch = &pools[0];
    let wp = &pools[1];
    Summary {
        total: all.len() as u32,
        primogems: all.len() as u64 * 160,
        fives: pools.iter().map(|p| p.fives.len() as u32).sum(),
        fours: pools.iter().map(|p| p.fours).sum(),
        won: ch.won,
        lost: ch.lost,
        weapon_won: wp.won,
        weapon_lost: wp.lost,
        early: all_fives.iter().filter(|(_, f)| f.early).count() as u32,
        // what the model expects for this mix of banners
        expected_early: if all_fives.is_empty() { expected_early(rates5(g, "character")) } else { all_fives.iter().map(|(p, _)| p.expected_early).sum::<f64>() / nf },
        avg_pity: if all_fives.is_empty() { 0.0 } else { all_fives.iter().map(|(_, f)| f.pity as f64).sum::<f64>() / nf },
        expected_avg: if all_fives.is_empty() { expected_pity(rates5(g, "character")) } else { all_fives.iter().map(|(p, _)| p.expected_avg).sum::<f64>() / nf },
        pools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::banners;
    use crate::game::Game;

    #[test]
    fn star_rail_light_cones() {
        let r = rates5(Game::Hsr, "weapon");
        assert_eq!((r.hard, r.soft), (80, 66));
        assert!(expected_pity(r) < expected_pity(rates5(Game::Hsr, "character")));
    }

    #[test]
    fn model_matches_known_numbers() {
        let c = rates5(Game::Genshin, "character");
        assert!((expected_pity(c) - 62.3).abs() < 0.5, "{}", expected_pity(c));
        assert_eq!(p_at(c, 90), 1.0);
        assert!((p_at(c, 74) - 0.066).abs() < 1e-9);
        assert!((expected_early(c) - 0.355).abs() < 0.01);
        let w = rates5(Game::Genshin, "weapon");
        assert!((expected_pity(w) - 53.7).abs() < 1.5, "{}", expected_pity(w));
        assert!(chance_within(c, 89, 1) == 1.0);
        assert!(chance_within(c, 0, 10) < 0.07);
    }

    fn wish(id: u64, t: &str, name: &str, rank: u8, gt: &str) -> Wish {
        Wish { id: id.to_string(), gacha_type: gt.into(), time: t.into(), name: name.into(), item_type: if rank == 5 { "Character".into() } else { "Weapon".into() }, rank, item_id: String::new() }
    }

    #[test]
    fn pity_and_fifty_fifty() {
        let list = banners::Banners::bundled();
        let mut ws = vec![];
        let mut id = 1000;
        // 76 wishes, the 76th is Diluc during the Mitya/Furina banners: a lost 50/50 (and in soft pity)
        for i in 1..=76 {
            id += 1;
            let (n, r) = if i == 76 { ("Diluc", 5) } else if i % 10 == 0 { ("Bennett", 4) } else { ("Slingshot", 3) };
            ws.push(wish(id, "2026-11-05 20:00:00", n, r, "301"));
        }
        // 20 more, then Furina on the second banner (400): guaranteed, early
        for i in 1..=20 {
            id += 1;
            let (n, r) = if i == 20 { ("Furina", 5) } else { ("Slingshot", 3) };
            ws.push(wish(id, "2026-11-06 20:00:00", n, r, if i == 20 { "400" } else { "301" }));
        }
        // 3 wishes after
        for _ in 0..3 {
            id += 1;
            ws.push(wish(id, "2026-11-07 20:00:00", "Slingshot", 3, "301"));
        }
        let s = summary(Game::Genshin, &ws, &list, "700000001");
        let c = &s.pools[0];
        assert_eq!(c.fives.len(), 2);
        assert_eq!((c.fives[0].pity, c.fives[0].result.as_str(), c.fives[0].early), (76, "lost", false));
        assert_eq!((c.fives[1].pity, c.fives[1].result.as_str(), c.fives[1].early), (20, "guaranteed", true));
        assert_eq!(c.pity5, 3);
        assert_eq!(c.guaranteed, Some(false));
        assert_eq!(c.zone, "normal");
        assert_eq!((s.won, s.lost), (0, 1));
        assert_eq!(s.early, 1);
        assert_eq!(s.primogems, 99 * 160);
    }
}
