//! Downloads wish history with the link the game saved. The link is valid for about a day;
//! after that the game needs to open Wish History once more to save a new one.
use crate::data::{Account, Wish};
use serde_json::Value;
use std::time::Duration;

/// Banner types to ask for. 301 also returns 400 (the second character banner).
pub const TYPES: [&str; 5] = ["301", "302", "500", "200", "100"];

#[derive(Debug)]
pub enum Error {
    /// the link has expired or isn't a wish link (retcode -100 / -101)
    Expired,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Expired => write!(f, "The wish link has expired. Open Wish History in the game once, then refresh."),
            Error::Other(s) => write!(f, "{}", s),
        }
    }
}

fn client() -> &'static reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(20)).user_agent("GenshinWishingCalculator").build().expect("http client"))
}

/// Pulls the parameters the history API needs out of the link the game saved.
pub fn api_base(url: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    let q = q.split('#').next().unwrap_or(q);
    let mut keep = vec![];
    let mut biz = "hk4e_global".to_string();
    for part in q.split('&') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        match k {
            "authkey" | "authkey_ver" | "sign_type" | "region" | "game_biz" => keep.push(format!("{}={}", k, v)),
            _ => {}
        }
        if k == "game_biz" {
            biz = v.to_string();
        }
    }
    if !keep.iter().any(|p| p.starts_with("authkey=")) {
        return None;
    }
    let host = if biz.starts_with("hk4e_cn") { "public-operation-hk4e.mihoyo.com" } else { "public-operation-hk4e-sg.hoyoverse.com" };
    Some(format!("https://{}/gacha_info/api/getGachaLog?{}&lang=en-us&size=20", host, keep.join("&")))
}

async fn page(base: &str, gacha_type: &str, end_id: &str) -> Result<Vec<Wish>, Error> {
    let url = format!("{}&gacha_type={}&end_id={}", base, gacha_type, end_id);
    for attempt in 0..4 {
        let r = client().get(&url).send().await.map_err(|e| Error::Other(format!("Couldn't reach HoYoverse: {}", e)))?;
        let v: Value = r.json().await.map_err(|e| Error::Other(format!("Unexpected answer: {}", e)))?;
        match v["retcode"].as_i64().unwrap_or(-1) {
            0 => {}
            -100 | -101 => return Err(Error::Expired),
            // "visit too frequently": wait and try again
            -110 if attempt < 3 => {
                tokio::time::sleep(Duration::from_millis(1200 * (attempt + 1))).await;
                continue;
            }
            c => return Err(Error::Other(format!("HoYoverse said: {} ({})", v["message"].as_str().unwrap_or("error"), c))),
        }
        let list = v["data"]["list"].as_array().cloned().unwrap_or_default();
        return Ok(list
            .iter()
            .map(|w| Wish {
                id: w["id"].as_str().unwrap_or_default().into(),
                gacha_type: w["gacha_type"].as_str().unwrap_or(gacha_type).into(),
                time: w["time"].as_str().unwrap_or_default().into(),
                name: w["name"].as_str().unwrap_or_default().into(),
                item_type: w["item_type"].as_str().unwrap_or_default().into(),
                rank: w["rank_type"].as_str().and_then(|r| r.parse().ok()).unwrap_or(3),
            })
            .collect());
    }
    Err(Error::Other("HoYoverse is busy, try again in a minute.".into()))
}

/// The UID the link belongs to (one tiny request).
pub async fn uid_of(base: &str) -> Result<Option<String>, Error> {
    for t in TYPES {
        let url = format!("{}&gacha_type={}&end_id=0", base.replace("&size=20", "&size=1"), t);
        let r = client().get(&url).send().await.map_err(|e| Error::Other(format!("Couldn't reach HoYoverse: {}", e)))?;
        let v: Value = r.json().await.map_err(|e| Error::Other(e.to_string()))?;
        match v["retcode"].as_i64().unwrap_or(-1) {
            0 => {}
            -100 | -101 => return Err(Error::Expired),
            _ => continue,
        }
        if let Some(uid) = v["data"]["list"][0]["uid"].as_str() {
            return Ok(Some(uid.to_string()));
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(None)
}

/// Downloads everything newer than what `acc` already has, banner by banner.
/// `progress(banner type, wishes so far)`.
pub async fn update(base: &str, acc: &mut Account, progress: &(dyn Fn(&str, usize) + Sync)) -> Result<usize, Error> {
    let mut added = 0;
    for t in TYPES {
        // 301 returns 400 wishes too, so stop at the newest of either
        let stop: Vec<String> = match t {
            "301" => ["301", "400"].iter().filter_map(|g| acc.newest(g).map(|w| w.id.clone())).collect(),
            _ => acc.newest(t).map(|w| vec![w.id.clone()]).unwrap_or_default(),
        };
        let stop_at = stop.iter().max_by(|a, b| (a.len(), a.as_str()).cmp(&(b.len(), b.as_str()))).cloned();
        let mut end_id = "0".to_string();
        let mut got: Vec<Wish> = vec![];
        loop {
            let list = page(base, t, &end_id).await?;
            if list.is_empty() {
                break;
            }
            let mut reached = false;
            for w in &list {
                if let Some(s) = &stop_at {
                    if (w.id.len(), w.id.as_str()) <= (s.len(), s.as_str()) {
                        reached = true;
                        break;
                    }
                }
                got.push(w.clone());
            }
            progress(t, got.len());
            if reached || list.len() < 20 {
                break;
            }
            end_id = list.last().map(|w| w.id.clone()).unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(350)).await;
        }
        added += acc.merge(got);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_api_link_from_game_link() {
        let u = "https://gs.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html?win_mode=fullscreen&authkey_ver=1&sign_type=2&auth_appid=webview_gacha&init_type=301&authkey=AB%2Bcd%3D&lang=de&device_type=pc&game_biz=hk4e_global&region=os_euro#/log";
        let b = super::api_base(u).unwrap();
        assert!(b.starts_with("https://public-operation-hk4e-sg.hoyoverse.com/gacha_info/api/getGachaLog?"));
        assert!(b.contains("authkey=AB%2Bcd%3D") && b.contains("region=os_euro") && b.contains("lang=en-us"));
        assert!(!b.contains("#"));
        let cn = super::api_base("https://webstatic.mihoyo.com/hk4e/event/e20190909gacha-v3/index.html?authkey=x&game_biz=hk4e_cn").unwrap();
        assert!(cn.contains("public-operation-hk4e.mihoyo.com"));
        assert!(super::api_base("https://example.com/?a=1").is_none());
    }
}
