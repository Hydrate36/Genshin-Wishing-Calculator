//! UIGF: the shared wish/warp history format other trackers read and write (v4.0, with v3 import
//! for Genshin and SRGF v1 import for Star Rail).
use crate::data::Wish;
use crate::game::{server_offset, Game};
use serde_json::{json, Value};

fn section(g: Game) -> &'static str {
    match g {
        Game::Genshin => "hk4e",
        Game::Hsr => "hkrpg",
    }
}

pub fn export(g: Game, uid: &str, wishes: &[Wish], app_version: &str) -> Value {
    let list: Vec<Value> = wishes
        .iter()
        .map(|w| {
            let mut v = json!({
                "gacha_type": w.gacha_type,
                "item_id": w.item_id,
                "count": "1",
                "time": w.time,
                "name": w.name,
                "item_type": w.item_type,
                "rank_type": w.rank.to_string(),
                "id": w.id,
            });
            if g == Game::Genshin {
                v["uigf_gacha_type"] = json!(if w.gacha_type == "400" { "301" } else { w.gacha_type.as_str() });
            } else {
                v["gacha_id"] = json!("");
            }
            v
        })
        .collect();
    let mut out = json!({
        "info": {
            "export_timestamp": chrono::Utc::now().timestamp().to_string(),
            "export_app": "Genshin Wishing Calculator",
            "export_app_version": app_version,
            "version": "v4.0",
        },
    });
    out[section(g)] = json!([{
        "uid": uid,
        "timezone": server_offset(uid),
        "lang": "en-us",
        "list": list,
    }]);
    out
}

fn read_list(list: &[Value]) -> Vec<Wish> {
    list.iter()
        .filter_map(|w| {
            let s = |k: &str| match &w[k] {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            };
            let id = s("id")?;
            let gacha_type = s("gacha_type").or_else(|| s("uigf_gacha_type"))?;
            Some(Wish {
                id,
                gacha_type,
                time: s("time").unwrap_or_default(),
                name: s("name").unwrap_or_default(),
                item_type: s("item_type").unwrap_or_default(),
                rank: s("rank_type").and_then(|r| r.parse().ok()).unwrap_or(3),
                item_id: s("item_id").unwrap_or_default(),
            })
        })
        .filter(|w| !w.name.is_empty() && !w.id.is_empty())
        .collect()
}

fn uid_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Returns (game, uid, wishes) for every account in the file, Genshin and Star Rail alike.
pub fn import(v: &Value) -> anyhow::Result<Vec<(Game, String, Vec<Wish>)>> {
    let mut out = vec![];
    for g in crate::game::ALL {
        if let Some(accts) = v[section(g)].as_array() {
            for a in accts {
                let Some(uid) = uid_of(&a["uid"]) else { continue };
                out.push((g, uid, read_list(a["list"].as_array().map(|l| l.as_slice()).unwrap_or(&[]))));
            }
        }
    }
    if out.is_empty() {
        if let Some(list) = v["list"].as_array() {
            // UIGF v2/v3 (Genshin) or SRGF v1 (Star Rail): one account, uid in info
            let g = if v["info"]["srgf_version"].is_string() { Game::Hsr } else { Game::Genshin };
            let uid = uid_of(&v["info"]["uid"]).or_else(|| list.first().and_then(|w| w["uid"].as_str().map(String::from))).unwrap_or_default();
            out.push((g, uid, read_list(list)));
        }
    }
    out.retain(|(_, u, l)| !u.is_empty() && u.bytes().all(|b| b.is_ascii_digit()) && !l.is_empty());
    if out.is_empty() {
        anyhow::bail!("No wishes or warps found in that file (it should be a UIGF or SRGF .json export).");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(gt: &str, name: &str, item_id: &str) -> Wish {
        Wish { id: "1".into(), gacha_type: gt.into(), time: "2026-01-01 10:00:00".into(), name: name.into(), item_type: "Character".into(), rank: 5, item_id: item_id.into() }
    }
    #[test]
    fn round_trips_both_games_and_old_formats() {
        let ws = vec![w("400", "Furina", "")];
        let e = export(Game::Genshin, "712345678", &ws, "2.1.0");
        assert_eq!(e["hk4e"][0]["list"][0]["uigf_gacha_type"], "301");
        let back = import(&e).unwrap();
        assert_eq!((back[0].0, back[0].1.as_str()), (Game::Genshin, "712345678"));
        assert_eq!(back[0].2, ws);
        let hs = vec![w("11", "Acheron", "1308")];
        let e = export(Game::Hsr, "700000001", &hs, "2.1.0");
        assert!(e["hk4e"].is_null());
        let back = import(&e).unwrap();
        assert_eq!((back[0].0, back[0].2[0].item_id.as_str()), (Game::Hsr, "1308"));
        let v3 = json!({"info": {"uid": "712345678", "uigf_version": "v3.0"}, "list": [{"id": "5", "gacha_type": "200", "time": "t", "name": "Amber", "item_type": "Character", "rank_type": "4"}]});
        assert_eq!(import(&v3).unwrap()[0].2[0].rank, 4);
        let srgf = json!({"info": {"uid": "700000001", "srgf_version": "v1.0"}, "list": [{"id": "5", "gacha_type": "1", "time": "t", "name": "Arlan", "item_type": "Character", "rank_type": "4", "item_id": "1008"}]});
        assert_eq!(import(&srgf).unwrap()[0].0, Game::Hsr);
        assert!(import(&json!({"x": 1})).is_err());
    }
}
