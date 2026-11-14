//! Banner list from paimon.moe's open data (github.com/MadeBaruna/paimon-moe, src/data/banners.js).
//! The file is a JavaScript object, turned into JSON here. A copy ships with the app for the
//! first start without internet; a fresh one is downloaded every few hours.
use chrono::{Duration, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SOURCE: &str = "https://raw.githubusercontent.com/MadeBaruna/paimon-moe/main/src/data/banners.js";
pub const BUNDLED: &str = include_str!("../data/banners.js");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Banner {
    /// "characters", "weapons", "chronicled", "standard", "beginners"
    pub kind: String,
    pub name: String,
    pub short_name: String,
    pub image: u32,
    pub start: String,
    pub end: String,
    pub color: String,
    pub version: String,
    /// 5★ rate-ups (for chronicled: every pickable 5★)
    pub featured: Vec<String>,
    /// 4★ rate-ups
    pub featured_rare: Vec<String>,
}

/// JavaScript object literal → JSON: drops comments and `export const x =`, quotes keys,
/// turns 'single' strings into "double" ones and removes trailing commas.
pub fn js_to_json(src: &str) -> String {
    let body = match (src.find('='), src.rfind('}')) {
        (Some(a), Some(b)) if b > a => &src[a + 1..=b],
        _ => src,
    };
    let c: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if ch == '\'' || ch == '"' {
            let q = ch;
            i += 1;
            out.push('"');
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    let n = c[i + 1];
                    if n == '\'' {
                        out.push('\'');
                    } else {
                        out.push('\\');
                        out.push(n);
                    }
                    i += 2;
                    continue;
                }
                if c[i] == '"' {
                    out.push('\\');
                }
                out.push(c[i]);
                i += 1;
            }
            out.push('"');
            i += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            let mut j = i;
            while j < c.len() && c[j].is_whitespace() {
                j += 1;
            }
            if c.get(j) == Some(&':') {
                out.push('"');
                out.push_str(&word);
                out.push('"');
            } else {
                out.push_str(&word);
            }
            continue;
        }
        if ch == ',' {
            let mut j = i + 1;
            while j < c.len() && c[j].is_whitespace() {
                j += 1;
            }
            // skip comments between a comma and the closing bracket
            while j + 1 < c.len() && c[j] == '/' && c[j + 1] == '/' {
                while j < c.len() && c[j] != '\n' {
                    j += 1;
                }
                while j < c.len() && c[j].is_whitespace() {
                    j += 1;
                }
            }
            if matches!(c.get(j), Some(']') | Some('}')) {
                i += 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

pub fn parse(src: &str) -> anyhow::Result<Vec<Banner>> {
    let v: Value = serde_json::from_str(&js_to_json(src))?;
    let mut out = vec![];
    for kind in ["beginners", "standard", "characters", "weapons", "chronicled"] {
        for b in v[kind].as_array().cloned().unwrap_or_default() {
            let s = |k: &str| b[k].as_str().unwrap_or_default().to_string();
            let list = |k: &str| b[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            out.push(Banner {
                kind: kind.into(),
                name: s("name"),
                short_name: s("shortName"),
                image: b["image"].as_u64().unwrap_or(1) as u32,
                start: s("start"),
                end: s("end"),
                color: s("color"),
                version: s("version"),
                featured: list("featured"),
                featured_rare: list("featuredRare"),
            });
        }
    }
    if out.iter().filter(|b| b.kind == "characters").count() < 10 {
        anyhow::bail!("banner list looks incomplete");
    }
    Ok(out)
}

/// The item id paimon.moe uses for a name ("Wolf's Gravestone" → "wolfs_gravestone").
pub fn item_id(name: &str) -> String {
    let lower = name.to_lowercase().replace(['\'', '’'], "");
    let mut out = String::with_capacity(lower.len());
    let mut last_us = true;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch);
            last_us = false;
        } else if !last_us {
            out.push('_');
            last_us = true;
        }
    }
    out.trim_end_matches('_').to_string()
}


/// Banner times are written as clock times. Version starts (06:00) and phase-2 ends (14:59) are
/// the same moment everywhere (Asia server time, UTC+8); phase changes (17:59 / 18:00) follow
/// each server's own clock.
pub fn banner_utc(t: &str, server: i64) -> Option<chrono::DateTime<Utc>> {
    let n = NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").ok()?;
    let hour = n.format("%H").to_string();
    let off = if hour == "17" || hour == "18" { server } else { 8 };
    Some(Utc.from_utc_datetime(&(n - Duration::hours(off))))
}

/// A wish time ("2026-11-05 21:14:03", server clock) as UTC.
pub fn wish_utc(t: &str, server: i64) -> Option<chrono::DateTime<Utc>> {
    let n = NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(Utc.from_utc_datetime(&(n - Duration::hours(server))))
}

/// Banners of `kind` running at `when`.
pub fn running_at<'a>(list: &'a [Banner], kind: &str, when: chrono::DateTime<Utc>, server: i64) -> Vec<&'a Banner> {
    list.iter()
        .filter(|b| b.kind == kind)
        .filter(|b| match (banner_utc(&b.start, server), banner_utc(&b.end, server)) {
            (Some(s), Some(e)) => s <= when && when <= e + Duration::minutes(1),
            _ => false,
        })
        .collect()
}

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    pub name: String,
    pub icon: String,
    /// second picture to try (Genshin's chronicled lists mix characters and weapons)
    pub alt: String,
}

#[derive(Serialize, Clone)]
pub struct Active {
    pub kind: String,
    /// banner name and artwork (Genshin only)
    pub name: String,
    pub art: String,
    pub color: String,
    pub version: String,
    pub featured: Vec<Item>,
    pub featured_rare: Vec<Item>,
    /// unix seconds
    pub ends: i64,
}

const PAIMON: &str = "https://paimon.moe/images";

fn genshin_item(id: &str, weapon: Option<bool>) -> Item {
    let c = format!("{}/characters/{}.png", PAIMON, id);
    let w = format!("{}/weapons/{}.png", PAIMON, id);
    let (icon, alt) = match weapon {
        Some(true) => (w, String::new()),
        Some(false) => (c, String::new()),
        None => (c, w),
    };
    Item { name: id.replace('_', " "), icon, alt }
}

/// Everything known about banners for both games.
#[derive(Clone, Default)]
pub struct Banners {
    pub genshin: Vec<Banner>,
    pub hsr: crate::hsr::Data,
}

impl Banners {
    pub fn bundled() -> Banners {
        let mut hsr = crate::hsr::parse_banners(crate::hsr::BUNDLED_BANNERS).unwrap_or_default();
        hsr.items = crate::hsr::parse_items_compact(crate::hsr::BUNDLED_ITEMS);
        Banners { genshin: parse(BUNDLED).unwrap_or_default(), hsr }
    }

    pub fn active(&self, g: crate::game::Game, server: i64) -> Vec<Active> {
        use crate::game::Game;
        match g {
            Game::Genshin => {
                let now = Utc::now();
                let mut out: Vec<Active> = ["characters", "weapons", "chronicled"]
                    .iter()
                    .flat_map(|k| running_at(&self.genshin, k, now, server))
                    .map(|b| {
                        let weapon = match b.kind.as_str() {
                            "weapons" => Some(true),
                            "characters" => Some(false),
                            _ => None,
                        };
                        let fives: Vec<&String> = if b.kind == "weapons" { b.featured.iter().take(2).collect() } else { b.featured.iter().collect() };
                        Active {
                            kind: b.kind.clone(),
                            name: b.name.clone(),
                            art: format!("{}/banners/{} {}.png", PAIMON, b.name, b.image).replace(' ', "%20").replace('\'', "%27"),
                            color: b.color.clone(),
                            version: b.version.clone(),
                            featured: fives.into_iter().map(|id| genshin_item(id, weapon)).collect(),
                            featured_rare: if b.kind == "chronicled" { vec![] } else { b.featured_rare.iter().map(|id| genshin_item(id, weapon)).collect() },
                            ends: banner_utc(&b.end, server).map(|e| e.timestamp()).unwrap_or(0),
                        }
                    })
                    .collect();
                out.sort_by_key(|a| match a.kind.as_str() {
                    "characters" => 0,
                    "weapons" => 1,
                    _ => 2,
                });
                out
            }
            Game::Hsr => {
                // Star Rail banners change at the server's midday; dates are server dates
                let local = Utc::now() + Duration::hours(server);
                let day = local.format("%Y-%m-%d").to_string();
                self.hsr
                    .active(&day)
                    .into_iter()
                    .map(|(kind, five, four, end)| {
                        let cone = kind == "weapons";
                        let ty = if cone { "Light Cone" } else { "Character" };
                        let item = |n: &String| Item { name: n.clone(), icon: self.hsr.icon(n, "", ty), alt: String::new() };
                        let ends = NaiveDateTime::parse_from_str(&format!("{} 11:59:00", end), "%Y-%m-%d %H:%M:%S")
                            .map(|n| (n - Duration::hours(server)).and_utc().timestamp())
                            .unwrap_or(0);
                        Active {
                            kind: kind.into(),
                            name: String::new(),
                            art: String::new(),
                            color: if cone { "#9fb8ff".into() } else { "#c9a7ff".into() },
                            version: String::new(),
                            featured: five.iter().map(item).collect(),
                            featured_rare: four.iter().map(item).collect(),
                            ends,
                        }
                    })
                    .collect()
            }
        }
    }
}

impl crate::stats::Catalog for Banners {
    fn featured(&self, g: crate::game::Game, pool: &str, w: &crate::data::Wish, server: i64) -> Option<bool> {
        use crate::game::Game;
        match g {
            Game::Genshin => {
                let kind = match pool {
                    "character" => "characters",
                    "weapon" => "weapons",
                    _ => return None,
                };
                let when = wish_utc(&w.time, server)?;
                let running = running_at(&self.genshin, kind, when, server);
                if running.is_empty() {
                    return None;
                }
                let id = item_id(&w.name);
                Some(running.iter().flat_map(|b| b.featured.iter().take(2)).any(|f| *f == id))
            }
            Game::Hsr => self.hsr.featured(pool, &w.name, &w.time),
        }
    }

    fn icon(&self, g: crate::game::Game, w: &crate::data::Wish) -> String {
        use crate::game::Game;
        match g {
            Game::Genshin => genshin_item(&item_id(&w.name), Some(w.item_type == "Weapon")).icon,
            Game::Hsr => self.hsr.icon(&w.name, &w.item_id, &w.item_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundled_list() {
        let l = parse(BUNDLED).unwrap();
        assert!(l.iter().any(|b| b.kind == "weapons" && b.featured.len() == 2));
        let venti = l.iter().find(|b| b.kind == "characters").unwrap();
        assert_eq!(venti.featured[0], "venti");
        // commented-out entries are skipped, apostrophes survive
        assert!(l.iter().any(|b| b.name == "Swan's Shadow in Silken Ice"));
    }

    #[test]
    fn ids_from_names() {
        assert_eq!(item_id("Wolf's Gravestone"), "wolfs_gravestone");
        assert_eq!(item_id("Kamisato Ayaka"), "kamisato_ayaka");
        assert_eq!(item_id("\"The Catch\""), "the_catch");
        assert_eq!(item_id("Sunny Morning Sleep-In"), "sunny_morning_sleep-in");
    }

    #[test]
    fn phases_follow_server_clock() {
        let l = parse(BUNDLED).unwrap();
        // Europe: a wish on 2026-11-05 evening is on the Mitya/Furina banners
        let t = wish_utc("2026-11-05 20:00:00", 1).unwrap();
        let r = running_at(&l, "characters", t, 1);
        assert!(r.iter().any(|b| b.featured.contains(&"mitya".to_string())), "{:?}", r.iter().map(|b| &b.name).collect::<Vec<_>>());
    }
}
