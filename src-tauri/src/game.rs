//! What differs between Genshin Impact and Honkai: Star Rail: where the game keeps its files,
//! which banner types exist, how they share pity, and where history and icons come from.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    Genshin,
    Hsr,
}

pub const ALL: [Game; 2] = [Game::Genshin, Game::Hsr];

impl Game {
    pub fn key(self) -> &'static str {
        match self {
            Game::Genshin => "genshin",
            Game::Hsr => "hsr",
        }
    }

    pub fn from_key(s: &str) -> Option<Game> {
        ALL.into_iter().find(|g| g.key() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Game::Genshin => "Genshin Impact",
            Game::Hsr => "Honkai: Star Rail",
        }
    }

    /// Game logs that name the install folder (global and Chinese clients).
    pub fn log_files(self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else { return vec![] };
        let low = home.join("AppData").join("LocalLow");
        let dirs: Vec<PathBuf> = match self {
            Game::Genshin => vec![low.join("miHoYo").join("Genshin Impact"), low.join("miHoYo").join("原神")],
            Game::Hsr => vec![low.join("Cognosphere").join("Star Rail"), low.join("miHoYo").join("崩坏：星穹铁道")],
        };
        let files: &[&str] = match self {
            Game::Genshin => &["output_log.txt", "output_log.txt.last"],
            Game::Hsr => &["Player.log", "Player-prev.log"],
        };
        dirs.iter().flat_map(|d| files.iter().map(move |f| d.join(f))).collect()
    }

    /// Names of the game's data folder (the one holding webCaches).
    pub fn data_names(self) -> &'static [&'static str] {
        match self {
            Game::Genshin => &["GenshinImpact_Data", "YuanShen_Data"],
            Game::Hsr => &["StarRail_Data"],
        }
    }

    /// Text that only this game's wish/warp links contain.
    pub fn link_marker(self) -> &'static [&'static str] {
        match self {
            Game::Genshin => &["hk4e", "genshin", "e20190909gacha"],
            Game::Hsr => &["hkrpg"],
        }
    }

    /// Banner types to download, in order.
    pub fn types(self) -> &'static [&'static str] {
        match self {
            Game::Genshin => &["301", "302", "500", "200", "100"],
            Game::Hsr => &["11", "12", "21", "22", "1", "2"],
        }
    }

    /// History service for a banner type: (host, path).
    pub fn api(self, cn: bool, gacha_type: &str) -> (&'static str, &'static str) {
        match self {
            Game::Genshin => (if cn { "public-operation-hk4e.mihoyo.com" } else { "public-operation-hk4e-sg.hoyoverse.com" }, "/gacha_info/api/getGachaLog"),
            Game::Hsr => (
                if cn { "public-operation-hkrpg.mihoyo.com" } else { "public-operation-hkrpg-sg.hoyoverse.com" },
                // collaboration warps have their own record list; the older /common/gacha_record/
                // address answers with empty lists for the regular warps
                if gacha_type == "21" || gacha_type == "22" { "/common/hkrpg_gacha_record/api/getLdGachaLog" } else { "/common/hkrpg_gacha_record/api/getGachaLog" },
            ),
        }
    }

    /// Which pity counter a banner type uses. Both character banners (and HSR's collaboration
    /// warps) share one counter and guarantee with the event banner of their kind.
    pub fn pool(self, gacha_type: &str) -> &'static str {
        match (self, gacha_type) {
            (Game::Genshin, "301" | "400") => "character",
            (Game::Genshin, "302") => "weapon",
            (Game::Genshin, "500") => "chronicled",
            (Game::Genshin, "100") => "beginner",
            (Game::Hsr, "11" | "21") => "character",
            (Game::Hsr, "12" | "22") => "weapon",
            (Game::Hsr, "2") => "beginner",
            _ => "standard",
        }
    }

    pub fn pools(self) -> &'static [&'static str] {
        match self {
            Game::Genshin => &["character", "weapon", "chronicled", "standard", "beginner"],
            Game::Hsr => &["character", "weapon", "standard", "beginner"],
        }
    }

    /// File name of an account's wishes (Genshin keeps the old names).
    pub fn account_file(self, uid: &str) -> String {
        match self {
            Game::Genshin => format!("{}.json", uid),
            Game::Hsr => format!("hsr-{}.json", uid),
        }
    }

    pub fn uid_from_file(self, file: &str) -> Option<String> {
        let stem = file.strip_suffix(".json")?;
        match self {
            Game::Genshin if !stem.contains('-') => Some(stem.into()),
            Game::Hsr => stem.strip_prefix("hsr-").map(String::from),
            _ => None,
        }
    }
}

/// Hours east of UTC for the server a UID plays on (same split for both games).
pub fn server_offset(uid: &str) -> i64 {
    let u = uid.trim();
    if u.starts_with("18") {
        return 8;
    }
    match u.chars().next() {
        Some('6') => -5,
        Some('7') => 1,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pools_and_files() {
        assert_eq!(Game::Hsr.pool("21"), "character");
        assert_eq!(Game::Hsr.pool("22"), "weapon");
        assert_eq!(Game::Hsr.pool("1"), "standard");
        assert_eq!(Game::Genshin.pool("400"), "character");
        assert_eq!(Game::Hsr.api(false, "22").1, "/common/hkrpg_gacha_record/api/getLdGachaLog");
        assert_eq!(Game::Genshin.uid_from_file("712345678.json").as_deref(), Some("712345678"));
        assert_eq!(Game::Genshin.uid_from_file("hsr-712345678.json"), None);
        assert_eq!(Game::Hsr.uid_from_file("hsr-712345678.json").as_deref(), Some("712345678"));
    }
}
