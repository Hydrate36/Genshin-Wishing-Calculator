//! UIGF: the shared wish-history format other trackers read and write (v4.0, with v3 import).
use crate::data::Wish;
use serde_json::{json, Value};

fn uigf_type(gacha_type: &str) -> &str {
    if gacha_type == "400" {
        "301"
    } else {
        gacha_type
    }
}

pub fn export(uid: &str, wishes: &[Wish], app_version: &str) -> Value {
    let list: Vec<Value> = wishes
        .iter()
        .map(|w| {
            json!({
                "uigf_gacha_type": uigf_type(&w.gacha_type),
                "gacha_type": w.gacha_type,
                "item_id": "",
                "count": "1",
                "time": w.time,
                "name": w.name,
                "item_type": w.item_type,
                "rank_type": w.rank.to_string(),
                "id": w.id,
            })
        })
        .collect();
    json!({
        "info": {
            "export_timestamp": chrono::Utc::now().timestamp().to_string(),
            "export_app": "Genshin Wishing Calculator",
            "export_app_version": app_version,
            "version": "v4.0",
        },
        "hk4e": [{
            "uid": uid,
            "timezone": crate::banners::server_offset(uid),
            "lang": "en-us",
            "list": list,
        }],
    })
}

fn read_list(uid: &str, list: &[Value]) -> Vec<Wish> {
    let _ = uid;
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
            })
        })
        .filter(|w| !w.name.is_empty() && !w.id.is_empty())
        .collect()
}

/// Returns (uid, wishes) for every Genshin account in the file.
pub fn import(v: &Value) -> anyhow::Result<Vec<(String, Vec<Wish>)>> {
    let mut out = vec![];
    if let Some(accts) = v["hk4e"].as_array() {
        for a in accts {
            let uid = match &a["uid"] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            out.push((uid.clone(), read_list(&uid, a["list"].as_array().map(|l| l.as_slice()).unwrap_or(&[]))));
        }
    } else if let Some(list) = v["list"].as_array() {
        // UIGF v2/v3: one account, uid in info
        let uid = match &v["info"]["uid"] {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => list.first().and_then(|w| w["uid"].as_str()).unwrap_or_default().to_string(),
        };
        out.push((uid.clone(), read_list(&uid, list)));
    }
    out.retain(|(u, l)| !u.is_empty() && u.bytes().all(|b| b.is_ascii_digit()) && !l.is_empty());
    if out.is_empty() {
        anyhow::bail!("No Genshin wishes found in that file (it should be a UIGF .json export).");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_v3() {
        let ws = vec![Wish { id: "1".into(), gacha_type: "400".into(), time: "2026-01-01 10:00:00".into(), name: "Furina".into(), item_type: "Character".into(), rank: 5 }];
        let e = export("712345678", &ws, "2.0.0");
        assert_eq!(e["hk4e"][0]["list"][0]["uigf_gacha_type"], "301");
        let back = import(&e).unwrap();
        assert_eq!(back[0].0, "712345678");
        assert_eq!(back[0].1, ws);
        let v3 = serde_json::json!({"info": {"uid": "712345678", "uigf_version": "v3.0"}, "list": [{"id": "5", "gacha_type": "200", "time": "t", "name": "Amber", "item_type": "Character", "rank_type": "4"}]});
        assert_eq!(import(&v3).unwrap()[0].1[0].rank, 4);
        assert!(import(&serde_json::json!({"x": 1})).is_err());
    }
}
