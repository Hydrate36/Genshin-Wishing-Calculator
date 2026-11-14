//! Wishes and settings on disk: one file per account (UID) in the app's data folder.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Wish {
    pub id: String,
    /// the game's banner type: 100, 200, 301, 400, 302, 500
    pub gacha_type: String,
    /// server clock, "2026-11-05 21:14:03"
    pub time: String,
    pub name: String,
    /// "Character" or "Weapon"
    pub item_type: String,
    pub rank: u8,
}

impl Wish {
    /// Ids grow over time; compare them as numbers.
    pub fn order(&self) -> (usize, &str) {
        (self.id.len(), self.id.as_str())
    }
}

/// Which pity counter a banner type uses (both character banners share one).
pub fn pool(gacha_type: &str) -> &'static str {
    match gacha_type {
        "301" | "400" => "character",
        "302" => "weapon",
        "500" => "chronicled",
        "100" => "beginner",
        _ => "standard",
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Account {
    pub uid: String,
    pub wishes: Vec<Wish>,
    /// unix seconds of the last successful download
    pub updated: i64,
}

impl Account {
    /// Adds wishes that aren't known yet; returns how many were new.
    pub fn merge(&mut self, new: Vec<Wish>) -> usize {
        let known: std::collections::HashSet<String> = self.wishes.iter().map(|w| w.id.clone()).collect();
        let before = self.wishes.len();
        for w in new {
            if !known.contains(&w.id) {
                self.wishes.push(w);
            }
        }
        self.wishes.sort_by(|a, b| a.order().cmp(&b.order()));
        self.wishes.dedup_by(|a, b| a.id == b.id);
        self.wishes.len() - before
    }

    /// Newest wish id already stored for one banner type.
    pub fn newest(&self, gacha_type: &str) -> Option<&Wish> {
        self.wishes.iter().filter(|w| w.gacha_type == gacha_type).max_by(|a, b| a.order().cmp(&b.order()))
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub selected: String,
    /// game folder picked by hand, when the log doesn't say where it is
    pub game_dir: String,
    pub notify: bool,
    /// per account and pool: the highest pity level already announced (0 none, 1 soft, 2 hard)
    pub announced: std::collections::HashMap<String, u8>,
    /// UTC hours of the user's server, before any account exists
    pub seen_help: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { selected: String::new(), game_dir: String::new(), notify: true, announced: Default::default(), seen_help: false }
    }
}

pub fn root() -> PathBuf {
    let d = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("GenshinWishingCalculator");
    let _ = std::fs::create_dir_all(d.join("accounts"));
    d
}

fn write_atomic(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn valid_uid(uid: &str) -> bool {
    !uid.is_empty() && uid.len() <= 12 && uid.bytes().all(|b| b.is_ascii_digit())
}

pub fn load_account(uid: &str) -> Account {
    if !valid_uid(uid) {
        return Account::default();
    }
    std::fs::read(root().join("accounts").join(format!("{}.json", uid)))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| Account { uid: uid.into(), ..Default::default() })
}

pub fn save_account(a: &Account) -> anyhow::Result<()> {
    if !valid_uid(&a.uid) {
        anyhow::bail!("bad UID");
    }
    write_atomic(&root().join("accounts").join(format!("{}.json", a.uid)), &serde_json::to_vec(a)?)?;
    Ok(())
}

pub fn list_accounts() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join("accounts"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(String::from))
                .filter(|u| valid_uid(u))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub fn load_settings() -> Settings {
    std::fs::read(root().join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    let _ = write_atomic(&root().join("settings.json"), &serde_json::to_vec(s).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(id: &str) -> Wish {
        Wish { id: id.into(), gacha_type: "301".into(), time: String::new(), name: "x".into(), item_type: "Weapon".into(), rank: 3 }
    }
    #[test]
    fn merge_keeps_order_and_skips_known() {
        let mut a = Account::default();
        assert_eq!(a.merge(vec![w("1700000000000000010"), w("999")]), 2);
        assert_eq!(a.merge(vec![w("999"), w("1700000000000000002")]), 1);
        let ids: Vec<&str> = a.wishes.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["999", "1700000000000000002", "1700000000000000010"]);
    }
}
