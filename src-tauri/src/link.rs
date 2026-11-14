//! Finds the wish-history link the game saves when you open Wish History in-game.
//! The game writes its install folder to its log; the link sits in the game's web cache.
use regex::Regex;
use std::path::{Path, PathBuf};

/// Where the game logs live (global and Chinese client).
fn log_files() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    let low = home.join("AppData").join("LocalLow").join("miHoYo");
    ["Genshin Impact", "原神"].iter().flat_map(|g| [low.join(g).join("output_log.txt"), low.join(g).join("output_log.txt.last")]).collect()
}

/// Game data folders (…/GenshinImpact_Data) named in the logs, newest log first.
pub fn data_dirs(manual: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    if !manual.is_empty() {
        let m = Path::new(manual);
        for d in [m.join("GenshinImpact_Data"), m.join("YuanShen_Data"), m.join("Genshin Impact game").join("GenshinImpact_Data"), m.to_path_buf()] {
            if d.join("webCaches").exists() && !out.contains(&d) {
                out.push(d);
            }
        }
    }
    let re = Regex::new(r"([A-Za-z]:[/\\][^\r\n:]*?(?:GenshinImpact_Data|YuanShen_Data))").unwrap();
    let mut logs: Vec<PathBuf> = log_files().into_iter().filter(|p| p.exists()).collect();
    logs.sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    for log in logs {
        let Ok(bytes) = std::fs::read(&log) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        for c in re.captures_iter(&text) {
            let d = PathBuf::from(c[1].replace('\\', "/"));
            if d.join("webCaches").exists() && !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// The game's web cache file (the newest version folder wins).
fn cache_file(data: &Path) -> Option<PathBuf> {
    let wc = data.join("webCaches");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut consider = |p: PathBuf| {
        if let Ok(t) = std::fs::metadata(&p).and_then(|m| m.modified()) {
            if best.as_ref().map(|(b, _)| t > *b).unwrap_or(true) {
                best = Some((t, p));
            }
        }
    };
    consider(wc.join("Cache").join("Cache_Data").join("data_2"));
    for e in std::fs::read_dir(&wc).ok()?.flatten() {
        consider(e.path().join("Cache").join("Cache_Data").join("data_2"));
    }
    best.map(|(_, p)| p)
}

/// The last wish-history link in a cache file's bytes.
pub fn find_in(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let re = Regex::new(r"https://[A-Za-z0-9./_-]+/(?:[A-Za-z0-9./_-]*)\?[^\s\x00\x01-\x1f\x7f-\u{ffff}]*authkey=[^\s\x00-\x1f\x7f-\u{ffff}]+").unwrap();
    re.find_iter(&text)
        .map(|m| m.as_str().to_string())
        .filter(|u| u.contains("gacha") || u.contains("getGachaLog"))
        .last()
}

pub struct Found {
    pub url: String,
    /// file modified time, unix seconds
    pub modified: i64,
}

/// The newest wish-history link on this PC, if the game has saved one.
pub fn find(manual: &str) -> Option<Found> {
    let mut best: Option<Found> = None;
    for d in data_dirs(manual) {
        let Some(f) = cache_file(&d) else { continue };
        let modified = std::fs::metadata(&f).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
        // the game keeps the file open; read a copy of whatever is there
        let Ok(bytes) = std::fs::read(&f) else { continue };
        if let Some(url) = find_in(&bytes) {
            if best.as_ref().map(|b| modified > b.modified).unwrap_or(true) {
                best = Some(Found { url, modified });
            }
        }
    }
    best
}

/// Is this a link we can use (copied by the user or found in the cache)?
pub fn looks_valid(url: &str) -> bool {
    url.starts_with("https://") && url.contains("authkey=")
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_last_link_in_binary_cache() {
        let mut b: Vec<u8> = vec![0, 1, 2, 3];
        b.extend_from_slice(b"1/0/https://webstatic-sea.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html?win_mode=fullscreen&authkey_ver=1&sign_type=2&authkey=OLD%2Bkey&lang=en&game_biz=hk4e_global#/log");
        b.extend_from_slice(&[0, 0, 0xff, 0x13]);
        b.extend_from_slice(b"1/0/https://gs.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html?authkey_ver=1&sign_type=2&auth_appid=webview_gacha&authkey=NEW%2Fkey%3D&lang=en&game_biz=hk4e_global&region=os_euro#/log");
        b.extend_from_slice(&[0, 9, 9]);
        let u = super::find_in(&b).unwrap();
        assert!(u.contains("authkey=NEW%2Fkey%3D"), "{}", u);
        assert!(u.ends_with("#/log"));
        assert!(super::find_in(b"nothing here https://example.com/?a=1").is_none());
    }
}
