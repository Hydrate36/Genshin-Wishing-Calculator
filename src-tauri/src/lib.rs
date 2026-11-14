//! Genshin Wishing Calculator: tracks pity on every banner from the wish history the game saves.
mod banners;
mod data;
mod gacha;
mod link;
mod stats;
mod uigf;

use serde::Serialize;
use serde_json::json;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

type R<T> = Result<T, String>;

struct AppState {
    banners: Mutex<Vec<banners::Banner>>,
    /// the history API link in use (never shown, never saved to disk)
    link: Mutex<Option<String>>,
    /// a refresh is running
    busy: Mutex<bool>,
    status: Mutex<Status>,
}

#[derive(Serialize, Clone, Default)]
struct Status {
    /// "none" (game never opened wish history), "ok", "expired", "error"
    link: String,
    message: String,
    /// unix seconds of the last successful refresh
    updated: i64,
    working: bool,
}

#[derive(Serialize)]
struct View {
    accounts: Vec<String>,
    uid: String,
    stats: Option<stats::Summary>,
    active: Vec<banners::Active>,
    status: Status,
    notify: bool,
    seen_help: bool,
    game_dir: String,
}

fn banners_file() -> std::path::PathBuf {
    data::root().join("banners.js")
}

fn load_banners() -> Vec<banners::Banner> {
    std::fs::read_to_string(banners_file())
        .ok()
        .and_then(|s| banners::parse(&s).ok())
        .unwrap_or_else(|| banners::parse(banners::BUNDLED).unwrap_or_default())
}

async fn refresh_banners(app: &AppHandle) {
    let Ok(r) = reqwest::get(banners::SOURCE).await else { return };
    let Ok(text) = r.text().await else { return };
    if let Ok(list) = banners::parse(&text) {
        let _ = std::fs::write(banners_file(), &text);
        *app.state::<AppState>().banners.lock().unwrap_or_else(|e| e.into_inner()) = list;
        let _ = app.emit("changed", ());
    }
}

fn view(state: &AppState) -> View {
    let s = data::load_settings();
    let accounts = data::list_accounts();
    let uid = if accounts.contains(&s.selected) { s.selected.clone() } else { accounts.first().cloned().unwrap_or_default() };
    let list = state.banners.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let stats = (!uid.is_empty()).then(|| stats::summary(&data::load_account(&uid).wishes, &list, &uid));
    // banner end times follow the server of the account shown (Europe before there is one)
    let server = if uid.is_empty() { 1 } else { banners::server_offset(&uid) };
    View {
        active: banners::active(&list, server),
        accounts,
        uid,
        stats,
        status: state.status.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        notify: s.notify,
        seen_help: s.seen_help,
        game_dir: s.game_dir,
    }
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let st = app.state::<AppState>();
    f(&mut st.status.lock().unwrap_or_else(|e| e.into_inner()));
    let _ = app.emit("changed", ());
}

/// Tells the player once when a banner reaches soft pity and when the next wish is certain.
fn announce(app: &AppHandle, uid: &str, sum: &stats::Summary) {
    let mut s = data::load_settings();
    let mut changed = false;
    for p in &sum.pools {
        if p.pool == "beginner" || p.total == 0 {
            continue;
        }
        let key = format!("{}:{}", uid, p.pool);
        let level = match p.zone.as_str() {
            "hard" => 2,
            "soft" => 1,
            _ => 0,
        };
        let before = *s.announced.get(&key).unwrap_or(&0);
        if level != before {
            s.announced.insert(key, level);
            changed = true;
            if level > before && s.notify {
                let banner = match p.pool.as_str() {
                    "character" => "Character banner",
                    "weapon" => "Weapon banner",
                    "chronicled" => "Chronicled Wish",
                    _ => "Standard banner",
                };
                let (title, body) = if level == 2 {
                    (format!("{}: hard pity", banner), format!("{} wishes since your last 5★. Your next wish is a guaranteed 5★.", p.pity5))
                } else {
                    (format!("{}: soft pity", banner), format!("{} wishes since your last 5★. From now on every wish has a much higher 5★ chance ({:.0}% on the next one).", p.pity5, p.chance_next * 100.0))
                };
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
    }
    if changed {
        data::save_settings(&s);
    }
}

/// Finds the newest link, downloads new wishes, saves them. `manual` is a link pasted by the user.
async fn do_refresh(app: &AppHandle, manual: Option<String>) -> R<usize> {
    let st = app.state::<AppState>();
    {
        let mut b = st.busy.lock().unwrap_or_else(|e| e.into_inner());
        if *b {
            return Ok(0);
        }
        *b = true;
    }
    set_status(app, |s| s.working = true);
    let res = refresh_inner(app, manual).await;
    *st.busy.lock().unwrap_or_else(|e| e.into_inner()) = false;
    match &res {
        Ok(_) => set_status(app, |s| {
            s.working = false;
            s.link = "ok".into();
            s.message.clear();
            s.updated = chrono::Utc::now().timestamp();
        }),
        Err((kind, msg)) => {
            let (k, m) = (kind.to_string(), msg.clone());
            set_status(app, move |s| {
                s.working = false;
                s.link = k;
                s.message = m;
            })
        }
    }
    res.map_err(|(_, m)| m)
}

async fn refresh_inner(app: &AppHandle, manual: Option<String>) -> Result<usize, (&'static str, String)> {
    let st = app.state::<AppState>();
    let settings = data::load_settings();
    // links to try: pasted one, the one in use, then the newest the game saved
    let mut tries: Vec<String> = vec![];
    if let Some(m) = manual.filter(|m| link::looks_valid(m)) {
        tries.push(m);
    }
    if let Some(cur) = st.link.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        tries.push(cur);
    }
    let dir = settings.game_dir.clone();
    if let Ok(Some(f)) = tokio::task::spawn_blocking(move || link::find(&dir)).await {
        if !tries.contains(&f.url) {
            tries.push(f.url);
        }
    }
    if tries.is_empty() {
        return Err(("none", "No wish link yet. Open the game, press F3 (Wish), then History, and refresh here.".into()));
    }
    let mut last_err = ("expired", "The wish link has expired. Open Wish History in the game once, then refresh.".to_string());
    for url in tries {
        let Some(base) = gacha::api_base(&url) else { continue };
        let uid = match gacha::uid_of(&base).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                last_err = ("error", "This account has no wishes yet.".into());
                continue;
            }
            Err(gacha::Error::Expired) => continue,
            Err(e) => {
                last_err = ("error", e.to_string());
                continue;
            }
        };
        let mut acc = data::load_account(&uid);
        acc.uid = uid.clone();
        let a = app.clone();
        let progress = move |t: &str, n: usize| {
            let _ = a.emit("progress", json!({ "banner": t, "count": n }));
        };
        match gacha::update(&base, &mut acc, &progress).await {
            Ok(added) => {
                acc.updated = chrono::Utc::now().timestamp();
                data::save_account(&acc).map_err(|e| ("error", e.to_string()))?;
                *st.link.lock().unwrap_or_else(|e| e.into_inner()) = Some(url);
                let mut s = data::load_settings();
                if s.selected.is_empty() || added > 0 {
                    s.selected = uid.clone();
                }
                data::save_settings(&s);
                let list = st.banners.lock().unwrap_or_else(|e| e.into_inner()).clone();
                announce(app, &uid, &stats::summary(&acc.wishes, &list, &uid));
                return Ok(added);
            }
            Err(gacha::Error::Expired) => continue,
            Err(e) => {
                // keep what was downloaded so far
                let _ = data::save_account(&acc);
                last_err = ("error", e.to_string());
            }
        }
    }
    *st.link.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Err(last_err)
}

/// Every few minutes: picks up new wishes while a link works, and a new link when the game saves one.
async fn auto_loop(app: AppHandle) {
    let dir = data::load_settings().game_dir;
    let mut last_cache = tokio::task::spawn_blocking(move || link::find(&dir).map(|f| f.modified)).await.ok().flatten().unwrap_or(0);
    let mut ticks = 0u64;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        ticks += 1;
        if ticks % 360 == 0 {
            refresh_banners(&app).await;
        }
        let has_link = app.state::<AppState>().link.lock().unwrap_or_else(|e| e.into_inner()).is_some();
        let dir = data::load_settings().game_dir;
        let newest = tokio::task::spawn_blocking(move || link::find(&dir).map(|f| f.modified)).await.ok().flatten().unwrap_or(0);
        // the game saved a fresh link, or 5 minutes went by with a working one
        if newest > last_cache || (has_link && ticks % 5 == 0) {
            last_cache = newest;
            let _ = do_refresh(&app, None).await;
        }
    }
}

#[tauri::command]
fn get_view(state: State<AppState>) -> View {
    view(&state)
}

#[tauri::command]
async fn refresh(app: AppHandle, link: Option<String>) -> R<usize> {
    do_refresh(&app, link).await
}

#[tauri::command]
fn select_account(uid: String) {
    let mut s = data::load_settings();
    s.selected = uid;
    data::save_settings(&s);
}

#[tauri::command]
fn set_prefs(notify: bool, seen_help: bool, game_dir: String) {
    let mut s = data::load_settings();
    s.notify = notify;
    s.seen_help = seen_help;
    s.game_dir = game_dir;
    data::save_settings(&s);
}

#[tauri::command]
fn export_uigf(uid: String, path: String) -> R<usize> {
    let acc = data::load_account(&uid);
    if acc.wishes.is_empty() {
        return Err("No wishes to export.".into());
    }
    let v = uigf::export(&uid, &acc.wishes, env!("CARGO_PKG_VERSION"));
    std::fs::write(&path, serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(acc.wishes.len())
}

#[tauri::command]
fn import_uigf(path: String) -> R<usize> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "That file isn't JSON.".to_string())?;
    let mut added = 0;
    let mut first = None;
    for (uid, wishes) in uigf::import(&v).map_err(|e| e.to_string())? {
        let mut acc = data::load_account(&uid);
        acc.uid = uid.clone();
        added += acc.merge(wishes);
        data::save_account(&acc).map_err(|e| e.to_string())?;
        first.get_or_insert(uid);
    }
    if let Some(uid) = first {
        let mut s = data::load_settings();
        s.selected = uid;
        data::save_settings(&s);
    }
    Ok(added)
}

/// `wish-tracker --check`: finds the game and its wish link and downloads history, without a window.
/// Prints only counts, never the link.
pub fn check() {
    let out = std::sync::Mutex::new(String::new());
    let say = |line: String| {
        println!("{}", line);
        let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
        o.push_str(&line);
        o.push('\n');
        let _ = std::fs::write(data::root().join("check.txt"), o.as_bytes());
    };
    let s = data::load_settings();
    let dirs = link::data_dirs(&s.game_dir);
    say(format!("game folders found: {}", dirs.len()));
    for d in &dirs {
        say(format!("  {}", d.display()));
    }
    let Some(f) = link::find(&s.game_dir) else {
        say(format!("no wish link saved by the game (open Wish History in the game once)"));
        return;
    };
    say(format!("wish link found (cache saved {} s ago)", chrono::Utc::now().timestamp() - f.modified));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let Some(base) = gacha::api_base(&f.url) else { say(format!("link has no authkey")); return };
        match gacha::uid_of(&base).await {
            Ok(Some(uid)) => {
                say(format!("account: UID ending {}", &uid[uid.len().saturating_sub(3)..]));
                let mut acc = data::load_account(&uid);
                acc.uid = uid.clone();
                let before = acc.wishes.len();
                match gacha::update(&base, &mut acc, &|t, n| println!("  banner {}: {} new", t, n)).await {
                    Ok(added) => {
                        acc.updated = chrono::Utc::now().timestamp();
                        let _ = data::save_account(&acc);
                        say(format!("wishes: {} before, {} new, {} total", before, added, acc.wishes.len()));
                        let list = load_banners();
                        let sum = stats::summary(&acc.wishes, &list, &uid);
                        for p in &sum.pools {
                            let unknown = p.fives.iter().filter(|f| f.result.is_empty()).count();
                            say(format!("  {:<10} wishes {:>4}  pity {:>2}/{}  zone {:<6} 5* {:>2} (won {} lost {} unknown {}) early {}", p.pool, p.total, p.pity5, p.hard, p.zone, p.fives.len(), p.won, p.lost, unknown, p.early));
                        }
                        say(format!("50/50: {} won {} lost; early {}/{}", sum.won, sum.lost, sum.early, sum.fives));
                    }
                    Err(e) => say(format!("download failed: {}", e)),
                }
            }
            Ok(None) => say("link works, account has no wishes".into()),
            Err(e) => say(format!("link not usable: {}", e)),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState { banners: Mutex::new(load_banners()), link: Mutex::new(None), busy: Mutex::new(false), status: Mutex::new(Status { link: "none".into(), ..Default::default() }) })
        .setup(|app| {
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                refresh_banners(&h).await;
                let _ = do_refresh(&h, None).await;
                auto_loop(h).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_view, refresh, select_account, set_prefs, export_uigf, import_uigf])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}

#[cfg(test)]
mod mock {
    use super::*;

    /// `cargo test dump_mock -- --ignored`: a believable history for UI previews.
    #[test]
    #[ignore]
    fn dump_mock() {
        let list = banners::parse(banners::BUNDLED).unwrap();
        let mut seed: u64 = 20261114;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let std5 = ["Diluc", "Jean", "Keqing", "Mona", "Qiqi", "Tighnari", "Dehya"];
        let mut wishes = vec![];
        let mut id: u64 = 1_700_000_000_000_000_000;
        let start = chrono::NaiveDate::from_ymd_opt(2025, 3, 1).unwrap().and_hms_opt(19, 0, 0).unwrap();
        let mut push = |gt: &str, n: usize, pool_name: &str, end_pity: u32, rnd: &mut dyn FnMut() -> f64| {
            let r5 = stats::rates5(pool_name);
            let (mut c5, mut c4) = (0u32, 0u32);
            let mut lost = false;
            let mut out = vec![];
            for i in 0..n {
                let t = start + chrono::Duration::minutes((i as i64) * (600 * 24 * 60 / n as i64));
                let time = t.format("%Y-%m-%d %H:%M:%S").to_string();
                c5 += 1;
                c4 += 1;
                let five = rnd() < stats::p_at(r5, c5);
                let (name, rank, ty) = if five {
                    c5 = 0;
                    let when = banners::wish_utc(&time, 1).unwrap();
                    let kind = if pool_name == "weapon" { "weapons" } else { "characters" };
                    let run = banners::running_at(&list, kind, when, 1);
                    let feat: Vec<String> = run.iter().flat_map(|b| b.featured.iter().take(if kind == "weapons" { 2 } else { 1 }).cloned()).collect();
                    let win = lost || rnd() < if pool_name == "weapon" { 0.75 } else { 0.5 };
                    let pick = if pool_name == "standard" || feat.is_empty() || !win { lost = pool_name != "standard" && !feat.is_empty() && !win; if pool_name == "weapon" { "Skyward Harp".to_string() } else { std5[(rnd() * 7.0) as usize].to_string() } } else { lost = false; titled(&feat[0]) };
                    (pick, 5, if pool_name == "weapon" { "Weapon" } else { "Character" })
                } else if c4 >= 10 || rnd() < 0.051 {
                    c4 = 0;
                    (["Bennett", "Xingqiu", "Xiangling", "Sucrose", "The Stringless"][(rnd() * 5.0) as usize].to_string(), 4, "Character")
                } else {
                    (["Slingshot", "Debate Club", "Cool Steel", "Black Tassel"][(rnd() * 4.0) as usize].to_string(), 3, "Weapon")
                };
                out.push((gt.to_string(), time, name, rank, ty.to_string()));
            }
            // top up so the banner ends at the wanted pity
            let last = out.last().map(|o| o.1.clone()).unwrap_or_default();
            if c5 < end_pity {
                for k in c5..end_pity {
                    let four = (end_pity - k) % 9 == 3;
                    out.push((gt.to_string(), last.clone(), if four { "Bennett".into() } else { "Slingshot".into() }, if four { 4 } else { 3 }, if four { "Character".into() } else { "Weapon".into() }));
                }
            }
            out
        };
        fn titled(id: &str) -> String {
            id.split('_').map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect::<Vec<_>>().join(" ")
        }
        let mut all = vec![];
        all.extend(push("301", 620, "character", 76, &mut rnd));
        all.extend(push("302", 140, "weapon", 22, &mut rnd));
        all.extend(push("200", 190, "standard", 41, &mut rnd));
        all.extend(push("500", 60, "chronicled", 60, &mut rnd));
        all.sort_by(|a, b| a.1.cmp(&b.1));
        for (gt, time, name, rank, ty) in all {
            id += 1;
            wishes.push(data::Wish { id: id.to_string(), gacha_type: gt, time, name, item_type: ty, rank });
        }
        let uid = "712345678";
        let s = stats::summary(&wishes, &list, uid);
        let v = json!({
            "accounts": [uid, "735555123"], "uid": uid, "stats": s,
            "active": banners::active(&list, 1),
            "status": { "link": "ok", "message": "", "updated": chrono::Utc::now().timestamp() - 140, "working": false },
            "notify": true, "seen_help": true, "game_dir": ""
        });
        std::fs::write("/home/claude/gwc/mock-view.json", serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
}
