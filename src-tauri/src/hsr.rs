//! Honkai: Star Rail banners and icons.
//! Banner dates: samsara (github.com/benlei/samsara-web, public/data/hsr-banners.yaml).
//! Icons by item id: StarRailRes (github.com/Mar-7th/StarRailRes), whose index maps names to ids.
use serde::Deserialize;
use std::collections::HashMap;

pub const BANNERS_URL: &str = "https://raw.githubusercontent.com/benlei/samsara-web/main/public/data/hsr-banners.yaml";
pub const CHARS_URL: &str = "https://raw.githubusercontent.com/Mar-7th/StarRailRes/master/index_min/en/characters.json";
pub const CONES_URL: &str = "https://raw.githubusercontent.com/Mar-7th/StarRailRes/master/index_min/en/light_cones.json";
pub const BUNDLED_BANNERS: &str = include_str!("../data/hsr-banners.yaml");
pub const BUNDLED_ITEMS: &str = include_str!("../data/hsr-items.json");
const ICONS: &str = "https://raw.githubusercontent.com/Mar-7th/StarRailRes/master/icon";

#[derive(Deserialize)]
struct Entry {
    name: String,
    #[serde(default)]
    dates: Vec<Dates>,
}

#[derive(Deserialize)]
struct Dates {
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    #[serde(default)]
    five_star_characters: Vec<Entry>,
    #[serde(default)]
    four_star_characters: Vec<Entry>,
    #[serde(default)]
    five_star_weapons: Vec<Entry>,
    #[serde(default)]
    four_star_weapons: Vec<Entry>,
}

/// One rate-up: an item and the days it was featured ("YYYY-MM-DD", end inclusive).
#[derive(Clone, Debug)]
pub struct Window {
    pub name: String,
    pub start: String,
    pub end: String,
}

/// An open-ended run ends at the latest three weeks after it starts.
fn close(start: &str, end: &str) -> String {
    if !end.is_empty() {
        return end.to_string();
    }
    chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").map(|d| (d + chrono::Duration::days(21)).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

fn windows(list: &[Entry]) -> Vec<Window> {
    list.iter()
        .flat_map(|e| e.dates.iter().filter(|d| d.start.len() == 10).map(move |d| Window { name: e.name.clone(), start: d.start.clone(), end: close(&d.start, &d.end) }))
        .collect()
}

#[derive(Clone, Default)]
pub struct Data {
    pub five_chars: Vec<Window>,
    pub four_chars: Vec<Window>,
    pub five_cones: Vec<Window>,
    pub four_cones: Vec<Window>,
    /// normalized name -> (id, is light cone)
    pub items: HashMap<String, (String, bool)>,
}

/// Names as different sources write them ("Topaz and Numby" / "Topaz & Numby", "March 7th (Preservation)").
pub fn norm(name: &str) -> String {
    let n = name.split(" (").next().unwrap_or(name).to_lowercase().replace(" and ", " & ");
    n.chars().filter(|c| c.is_alphanumeric() || *c == '&').collect()
}

pub fn parse_banners(yaml: &str) -> anyhow::Result<Data> {
    let f: File = serde_yaml::from_str(yaml)?;
    let d = Data {
        five_chars: windows(&f.five_star_characters),
        four_chars: windows(&f.four_star_characters),
        five_cones: windows(&f.five_star_weapons),
        four_cones: windows(&f.four_star_weapons),
        items: HashMap::new(),
    };
    if d.five_chars.len() < 20 {
        anyhow::bail!("banner list looks incomplete");
    }
    Ok(d)
}

/// Bundled compact index: {"characters": {name: [id, rarity]}, "light_cones": {...}}
pub fn parse_items_compact(json: &str) -> HashMap<String, (String, bool)> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let mut out = HashMap::new();
    for (key, cone) in [("characters", false), ("light_cones", true)] {
        if let Some(m) = v[key].as_object() {
            for (name, x) in m {
                if let Some(id) = x[0].as_str() {
                    out.entry(norm(name)).or_insert((id.to_string(), cone));
                }
            }
        }
    }
    out
}

/// StarRailRes index_min file ({id: {id, name, ...}}).
pub fn parse_items_index(json: &str, cone: bool, into: &mut HashMap<String, (String, bool)>) -> usize {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let mut n = 0;
    if let Some(m) = v.as_object() {
        for (id, x) in m {
            if let Some(name) = x["name"].as_str().filter(|n| !n.starts_with('{')) {
                into.entry(norm(name)).or_insert((id.clone(), cone));
                n += 1;
            }
        }
    }
    n
}

pub fn icon_by_id(id: &str, cone: bool) -> String {
    format!("{}/{}/{}.png", ICONS, if cone { "light_cone" } else { "character" }, id)
}

impl Data {
    pub fn icon(&self, name: &str, item_id: &str, item_type: &str) -> String {
        let cone = item_type.contains("Light Cone");
        if !item_id.is_empty() {
            return icon_by_id(item_id, cone);
        }
        self.items.get(&norm(name)).map(|(id, c)| icon_by_id(id, *c)).unwrap_or_default()
    }

    fn running<'a>(list: &'a [Window], day: &str) -> impl Iterator<Item = &'a Window> + 'a {
        let day = day.to_string();
        list.iter().filter(move |w| w.start.as_str() <= day.as_str() && day.as_str() <= w.end.as_str())
    }

    /// Was this 5★ featured on its banner when it was pulled? None when the day isn't covered.
    pub fn featured(&self, pool: &str, name: &str, time: &str) -> Option<bool> {
        let day = time.get(..10)?;
        let list = match pool {
            "character" => &self.five_chars,
            "weapon" => &self.five_cones,
            _ => return None,
        };
        let running: Vec<&Window> = Self::running(list, day).collect();
        if running.is_empty() {
            return None;
        }
        let n = norm(name);
        Some(running.iter().any(|w| norm(&w.name) == n))
    }

    /// Rate-ups running on `day` ("YYYY-MM-DD", server date): (kind, 5★ names, 4★ names, last day).
    pub fn active(&self, day: &str) -> Vec<(&'static str, Vec<String>, Vec<String>, String)> {
        let mut out = vec![];
        for (kind, five, four) in [("characters", &self.five_chars, &self.four_chars), ("weapons", &self.five_cones, &self.four_cones)] {
            let fives: Vec<&Window> = Self::running(five, day).collect();
            if fives.is_empty() {
                continue;
            }
            let mut names: Vec<String> = fives.iter().map(|w| w.name.clone()).collect();
            names.dedup();
            let rare: Vec<String> = Self::running(four, day).map(|w| w.name.clone()).collect();
            let end = fives.iter().map(|w| w.end.clone()).min().unwrap_or_default();
            out.push((kind, names, rare, end));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> Data {
        let mut d = parse_banners(BUNDLED_BANNERS).unwrap();
        d.items = parse_items_compact(BUNDLED_ITEMS);
        d
    }

    #[test]
    fn featured_by_date() {
        let d = data();
        // Seele's first run
        assert_eq!(d.featured("character", "Seele", "2023-05-01 12:00:00"), Some(true));
        assert_eq!(d.featured("character", "Bronya", "2023-05-01 12:00:00"), Some(false));
        assert_eq!(d.featured("character", "Seele", "2020-01-01 12:00:00"), None);
        assert_eq!(d.featured("standard", "Seele", "2023-05-01 12:00:00"), None);
    }

    #[test]
    fn icons_for_banner_names_and_ids() {
        let d = data();
        assert!(d.icon("Topaz and Numby", "", "Character").ends_with("/character/1112.png"), "{}", d.icon("Topaz and Numby", "", "Character"));
        assert!(d.icon("March 7th (Preservation)", "", "Character").ends_with("/character/1001.png"));
        assert!(d.icon("Arrows", "", "Light Cone").contains("/light_cone/"));
        assert!(d.icon("whatever", "1003", "Character").ends_with("/character/1003.png"));
        // every name samsara lists has an icon
        for w in d.five_chars.iter().chain(&d.four_chars).chain(&d.five_cones).chain(&d.four_cones) {
            assert!(!d.icon(&w.name, "", "").is_empty(), "no icon for {}", w.name);
        }
    }

    #[test]
    fn active_on_a_day() {
        let d = data();
        let a = d.active("2023-05-01");
        assert!(a.iter().any(|(k, five, _, _)| *k == "characters" && five.contains(&"Seele".to_string())));
        assert!(a.iter().any(|(k, _, _, _)| *k == "weapons"));
    }
}
