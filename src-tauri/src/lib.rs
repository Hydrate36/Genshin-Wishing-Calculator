//! Genshin Wishing Calculator: tracks pity on every banner of Genshin Impact and Honkai: Star Rail
//! from the wish/warp history the games save.
mod banners;
mod data;
mod gacha;
mod game;
mod hsr;
mod link;
mod stats;
mod uigf;

use game::Game;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

type R<T> = Result<T, String>;

struct AppState {
    banners: Mutex<banners::Banners>,
    /// per game: the history link in use (never shown, never saved to disk)
    link: Mutex<HashMap<Game, String>>,
    /// per game: a refresh is running
    busy: Mutex<HashMap<Game, bool>>,
    status: Mutex<HashMap<Game, Status>>,
}

#[derive(Serialize, Clone)]
struct Status {
    /// "none" (game never opened its history), "ok", "expired", "error"
    link: String,
    message: String,
    /// unix seconds of the last successful refresh
    updated: i64,
    working: bool,
}

impl Default for Status {
    fn default() -> Self {
        Status { link: "none".into(), message: String::new(), updated: 0, working: false }
    }
}

#[derive(Serialize)]
struct View {
    game: Game,
    /// which games have their history on this PC (an install was found or an account exists)
    games: Vec<Game>,
    accounts: Vec<String>,
    uid: String,
    stats: Option<stats::Summary>,
    active: Vec<banners::Active>,
    status: Status,
    notify: bool,
    seen_help: bool,
    game_dir: String,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ── banner data: bundled copies, cached downloads, refreshed every few hours ──

fn cache(name: &str) -> std::path::PathBuf {
    data::root().join(name)
}

fn load_banners() -> banners::Banners {
    let mut b = banners::Banners::bundled();
    if let Some(list) = std::fs::read_to_string(cache("banners.js")).ok().and_then(|s| banners::parse(&s).ok()) {
        b.genshin = list;
    }
    if let Some(mut d) = std::fs::read_to_string(cache("hsr-banners.yaml")).ok().and_then(|s| hsr::parse_banners(&s).ok()) {
        d.items = std::mem::take(&mut b.hsr.items);
        b.hsr = d;
    }
    for (f, cone) in [("hsr-characters.json", false), ("hsr-light_cones.json", true)] {
        if let Ok(s) = std::fs::read_to_string(cache(f)) {
            hsr::parse_items_index(&s, cone, &mut b.hsr.items);
        }
    }
    b
}

async fn fetch(url: &str) -> Option<String> {
    let r = reqwest::get(url).await.ok()?;
    if !r.status().is_success() {
        return None;
    }
    r.text().await.ok()
}

async fn refresh_banners(app: &AppHandle) {
    let st = app.state::<AppState>();
    if let Some(text) = fetch(banners::SOURCE).await {
        if let Ok(list) = banners::parse(&text) {
            let _ = std::fs::write(cache("banners.js"), &text);
            lock(&st.banners).genshin = list;
        }
    }
    if let Some(text) = fetch(hsr::BANNERS_URL).await {
        if let Ok(mut d) = hsr::parse_banners(&text) {
            let _ = std::fs::write(cache("hsr-banners.yaml"), &text);
            let mut b = lock(&st.banners);
            d.items = std::mem::take(&mut b.hsr.items);
            b.hsr = d;
        }
    }
    for (url, f, cone) in [(hsr::CHARS_URL, "hsr-characters.json", false), (hsr::CONES_URL, "hsr-light_cones.json", true)] {
        if let Some(text) = fetch(url).await {
            if hsr::parse_items_index(&text, cone, &mut lock(&st.banners).hsr.items) > 0 {
                let _ = std::fs::write(cache(f), &text);
            }
        }
    }
    let _ = app.emit("changed", ());
}

// ── what the window shows ──

fn games_here() -> Vec<Game> {
    let s = data::load_settings();
    game::ALL.into_iter().filter(|g| !data::list_accounts(*g).is_empty() || !link::data_dirs(*g, s.dir(*g)).is_empty()).collect()
}

fn view(state: &AppState, g: Game) -> View {
    let s = data::load_settings();
    let accounts = data::list_accounts(g);
    let sel = s.selected(g).to_string();
    let uid = if accounts.contains(&sel) { sel } else { accounts.first().cloned().unwrap_or_default() };
    let b = lock(&state.banners).clone();
    let stats = (!uid.is_empty()).then(|| stats::summary(g, &data::load_account(g, &uid).wishes, &b, &uid));
    // banner end times follow the server of the account shown (Europe before there is one)
    let server = if uid.is_empty() { 1 } else { game::server_offset(&uid) };
    let mut games = games_here();
    if !games.contains(&g) {
        games.push(g);
        games.sort_by_key(|x| x.key() != "genshin");
    }
    View {
        game: g,
        games,
        active: b.active(g, server),
        accounts,
        uid,
        stats,
        status: lock(&state.status).get(&g).cloned().unwrap_or_default(),
        notify: s.notify,
        seen_help: s.seen_help,
        game_dir: s.dir(g).to_string(),
    }
}

fn set_status(app: &AppHandle, g: Game, f: impl FnOnce(&mut Status)) {
    let st = app.state::<AppState>();
    f(lock(&st.status).entry(g).or_default());
    let _ = app.emit("changed", ());
}

fn pool_label(g: Game, pool: &str) -> &'static str {
    match (g, pool) {
        (Game::Genshin, "character") => "Character banner",
        (Game::Genshin, "weapon") => "Weapon banner",
        (Game::Genshin, "chronicled") => "Chronicled Wish",
        (Game::Genshin, _) => "Standard banner",
        (Game::Hsr, "character") => "Star Rail character warp",
        (Game::Hsr, "weapon") => "Star Rail Light Cone warp",
        (Game::Hsr, _) => "Star Rail Stellar Warp",
    }
}

/// Tells the player once when a banner reaches soft pity and when the next pull is certain.
fn announce(app: &AppHandle, g: Game, uid: &str, sum: &stats::Summary) {
    let mut s = data::load_settings();
    let mut changed = false;
    let pull = if g == Game::Hsr { "warp" } else { "wish" };
    for p in &sum.pools {
        if p.pool == "beginner" || p.total == 0 {
            continue;
        }
        // Genshin keys stay as they were before Star Rail was added
        let key = if g == Game::Genshin { format!("{}:{}", uid, p.pool) } else { format!("{}:{}:{}", g.key(), uid, p.pool) };
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
                let banner = pool_label(g, &p.pool);
                let (title, body) = if level == 2 {
                    (format!("{}: hard pity", banner), format!("{} {}s since your last 5★. Your next {} is a guaranteed 5★.", p.pity5, pull, pull))
                } else {
                    (format!("{}: soft pity", banner), format!("{} {}s since your last 5★. From now on every {} has a much higher 5★ chance ({:.0}% on the next one).", p.pity5, pull, pull, p.chance_next * 100.0))
                };
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
    }
    if changed {
        data::save_settings(&s);
    }
}

// ── downloading history ──

/// Finds the newest link, downloads new wishes/warps, saves them. `manual` is a link pasted by the user.
async fn do_refresh(app: &AppHandle, g: Game, manual: Option<String>) -> R<usize> {
    let st = app.state::<AppState>();
    {
        let mut b = lock(&st.busy);
        if *b.get(&g).unwrap_or(&false) {
            return Ok(0);
        }
        b.insert(g, true);
    }
    set_status(app, g, |s| s.working = true);
    let res = refresh_inner(app, g, manual).await;
    lock(&st.busy).insert(g, false);
    match &res {
        Ok(_) => set_status(app, g, |s| {
            s.working = false;
            s.link = "ok".into();
            s.message.clear();
            s.updated = chrono::Utc::now().timestamp();
        }),
        Err((kind, msg)) => {
            let (k, m) = (kind.to_string(), msg.clone());
            set_status(app, g, move |s| {
                s.working = false;
                s.link = k;
                s.message = m;
            })
        }
    }
    res.map_err(|(_, m)| m)
}

fn expired_text(g: Game) -> String {
    match g {
        Game::Genshin => "The wish link has expired. Open Wish History in the game once, then refresh.".into(),
        Game::Hsr => "The warp link has expired. Open Warp, then Records, in the game once, then refresh.".into(),
    }
}

async fn refresh_inner(app: &AppHandle, g: Game, manual: Option<String>) -> Result<usize, (&'static str, String)> {
    let st = app.state::<AppState>();
    let settings = data::load_settings();
    // links to try: pasted one, the one in use, then the newest the game saved
    let mut tries: Vec<String> = vec![];
    if let Some(m) = manual.filter(|m| link::looks_valid(m)) {
        tries.push(m);
    }
    if let Some(cur) = lock(&st.link).get(&g).cloned() {
        tries.push(cur);
    }
    let dir = settings.dir(g).to_string();
    if let Ok(Some(f)) = tokio::task::spawn_blocking(move || link::find(g, &dir)).await {
        if !tries.contains(&f.url) {
            tries.push(f.url);
        }
    }
    if tries.is_empty() {
        return Err(("none", match g {
            Game::Genshin => "No wish link yet. Open the game, press F3 (Wish), then History, and refresh here.".into(),
            Game::Hsr => "No warp link yet. Open the game, open Warp, then Records, and refresh here.".into(),
        }));
    }
    let mut last_err = ("expired", expired_text(g));
    for url in tries {
        let Some(base) = gacha::api_base(g, &url) else { continue };
        let uid = match gacha::uid_of(&base).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                last_err = ("error", "This account has no history yet.".into());
                continue;
            }
            Err(gacha::Error::Expired) => continue,
            Err(e) => {
                last_err = ("error", e.to_string());
                continue;
            }
        };
        let mut acc = data::load_account(g, &uid);
        acc.uid = uid.clone();
        let a = app.clone();
        let progress = move |t: &str, n: usize| {
            let _ = a.emit("progress", json!({ "game": g, "banner": t, "count": n }));
        };
        match gacha::update(&base, &mut acc, &progress).await {
            Ok(added) => {
                acc.updated = chrono::Utc::now().timestamp();
                data::save_account(g, &acc).map_err(|e| ("error", e.to_string()))?;
                lock(&st.link).insert(g, url);
                let mut s = data::load_settings();
                if s.selected(g).is_empty() || added > 0 {
                    s.set_selected(g, uid.clone());
                }
                data::save_settings(&s);
                let b = lock(&st.banners).clone();
                announce(app, g, &uid, &stats::summary(g, &acc.wishes, &b, &uid));
                return Ok(added);
            }
            Err(gacha::Error::Expired) => continue,
            Err(e) => {
                // keep what was downloaded so far
                let _ = data::save_account(g, &acc);
                last_err = ("error", e.to_string());
            }
        }
    }
    lock(&st.link).remove(&g);
    Err(last_err)
}

fn newest_link_time(g: Game) -> i64 {
    let dir = data::load_settings().dir(g).to_string();
    link::find(g, &dir).map(|f| f.modified).unwrap_or(0)
}

/// Every minute: picks up a new link when a game saves one; every 5 minutes, new pulls while a link works.
async fn auto_loop(app: AppHandle) {
    let mut last: HashMap<Game, i64> = HashMap::new();
    for g in game::ALL {
        last.insert(g, tokio::task::spawn_blocking(move || newest_link_time(g)).await.unwrap_or(0));
    }
    let mut ticks = 0u64;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        ticks += 1;
        if ticks % 360 == 0 {
            refresh_banners(&app).await;
        }
        for g in game::ALL {
            let has_link = lock(&app.state::<AppState>().link).contains_key(&g);
            let newest = tokio::task::spawn_blocking(move || newest_link_time(g)).await.unwrap_or(0);
            if newest > *last.get(&g).unwrap_or(&0) || (has_link && ticks % 5 == 0) {
                last.insert(g, newest);
                let _ = do_refresh(&app, g, None).await;
            }
        }
    }
}

// ── commands ──

fn game_or_current(g: Option<String>) -> Game {
    g.and_then(|k| Game::from_key(&k)).unwrap_or_else(|| data::load_settings().current())
}

#[tauri::command]
fn get_view(state: State<AppState>, game: Option<String>) -> View {
    view(&state, game_or_current(game))
}

#[tauri::command]
fn set_game(game: String) {
    if let Some(g) = Game::from_key(&game) {
        let mut s = data::load_settings();
        s.game = g.key().into();
        data::save_settings(&s);
    }
}

#[tauri::command]
async fn refresh(app: AppHandle, game: Option<String>, link: Option<String>) -> R<usize> {
    do_refresh(&app, game_or_current(game), link).await
}

#[tauri::command]
fn select_account(game: Option<String>, uid: String) {
    let g = game_or_current(game);
    let mut s = data::load_settings();
    s.set_selected(g, uid);
    data::save_settings(&s);
}

#[tauri::command]
fn set_prefs(game: Option<String>, notify: bool, seen_help: bool, game_dir: String) {
    let g = game_or_current(game);
    let mut s = data::load_settings();
    s.notify = notify;
    s.seen_help = seen_help;
    match g {
        Game::Genshin => s.game_dir = game_dir,
        Game::Hsr => s.hsr_dir = game_dir,
    }
    data::save_settings(&s);
}

#[tauri::command]
fn export_uigf(game: Option<String>, uid: String, path: String) -> R<usize> {
    let g = game_or_current(game);
    let acc = data::load_account(g, &uid);
    if acc.wishes.is_empty() {
        return Err("Nothing to export.".into());
    }
    let v = uigf::export(g, &uid, &acc.wishes, env!("CARGO_PKG_VERSION"));
    std::fs::write(&path, serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(acc.wishes.len())
}

#[tauri::command]
fn import_uigf(path: String) -> R<usize> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "That file isn't JSON.".to_string())?;
    let mut added = 0;
    let mut s = data::load_settings();
    for (g, uid, wishes) in uigf::import(&v).map_err(|e| e.to_string())? {
        let mut acc = data::load_account(g, &uid);
        acc.uid = uid.clone();
        added += acc.merge(wishes);
        data::save_account(g, &acc).map_err(|e| e.to_string())?;
        s.set_selected(g, uid);
        s.game = g.key().into();
    }
    data::save_settings(&s);
    Ok(added)
}

/// `wish-tracker --check`: finds both games and their links and downloads history, without a window.
/// Prints only counts, never a link. Also written to check.txt in the app's data folder.
pub fn check() {
    let out = Mutex::new(String::new());
    let say = |line: String| {
        println!("{}", line);
        let mut o = lock(&out);
        o.push_str(&line);
        o.push('\n');
        let _ = std::fs::write(data::root().join("check.txt"), o.as_bytes());
    };
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let b = load_banners();
    for g in game::ALL {
        say(format!("== {}", g.name()));
        let s = data::load_settings();
        let dirs = link::data_dirs(g, s.dir(g));
        say(format!("game folders found: {}", dirs.len()));
        for d in &dirs {
            say(format!("  {}", d.display()));
        }
        let Some(f) = link::find(g, s.dir(g)) else {
            say("no link saved by the game yet".into());
            continue;
        };
        say(format!("link found (cache saved {} s ago)", chrono::Utc::now().timestamp() - f.modified));
        rt.block_on(async {
            let Some(base) = gacha::api_base(g, &f.url) else {
                say("link has no authkey".into());
                return;
            };
            match gacha::uid_of(&base).await {
                Ok(Some(uid)) => {
                    say(format!("account: UID ending {}", &uid[uid.len().saturating_sub(3)..]));
                    let mut acc = data::load_account(g, &uid);
                    acc.uid = uid.clone();
                    let before = acc.wishes.len();
                    match gacha::update(&base, &mut acc, &|t, n| println!("  banner {}: {} new", t, n)).await {
                        Ok(added) => {
                            acc.updated = chrono::Utc::now().timestamp();
                            let _ = data::save_account(g, &acc);
                            say(format!("pulls: {} before, {} new, {} total", before, added, acc.wishes.len()));
                            let sum = stats::summary(g, &acc.wishes, &b, &uid);
                            for p in &sum.pools {
                                let unknown = p.fives.iter().filter(|f| f.result.is_empty()).count();
                                let no_icon = p.fives.iter().filter(|f| f.icon.is_empty()).count();
                                say(format!("  {:<10} pulls {:>4}  pity {:>2}/{}  zone {:<6} 5* {:>2} (won {} lost {} unknown {}) early {} no-icon {}", p.pool, p.total, p.pity5, p.hard, p.zone, p.fives.len(), p.won, p.lost, unknown, p.early, no_icon));
                            }
                            say(format!("50/50: {} won {} lost; early {}/{}", sum.won, sum.lost, sum.early, sum.fives));
                        }
                        Err(e) => say(format!("download failed: {}", e)),
                    }
                }
                Ok(None) => say("link works, account has no history".into()),
                Err(e) => say(format!("link not usable: {}", e)),
            }
        });
    }
    let server = 1;
    for g in game::ALL {
        let a = b.active(g, server);
        say(format!("{} active banners: {}", g.name(), a.iter().map(|x| format!("{} [{}]", x.kind, x.featured.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ")));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState { banners: Mutex::new(load_banners()), link: Mutex::new(HashMap::new()), busy: Mutex::new(HashMap::new()), status: Mutex::new(HashMap::new()) })
        .setup(|app| {
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                refresh_banners(&h).await;
                for g in game::ALL {
                    let _ = do_refresh(&h, g, None).await;
                }
                auto_loop(h).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_view, set_game, refresh, select_account, set_prefs, export_uigf, import_uigf])
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
        let cat = banners::Banners::bundled();
        let list = cat.genshin.clone();
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
            let r5 = stats::rates5(Game::Genshin, pool_name);
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
            wishes.push(data::Wish { id: id.to_string(), gacha_type: gt, time, name, item_type: ty, rank, item_id: String::new() });
        }
        let uid = "712345678";
        let s = stats::summary(Game::Genshin, &wishes, &cat, uid);
        let v = json!({
            "game": "genshin", "games": ["genshin", "hsr"],
            "accounts": [uid, "735555123"], "uid": uid, "stats": s,
            "active": cat.active(Game::Genshin, 1),
            "status": { "link": "ok", "message": "", "updated": chrono::Utc::now().timestamp() - 140, "working": false },
            "notify": true, "seen_help": true, "game_dir": ""
        });
        std::fs::write("/home/claude/gwc/mock-view.json", serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    /// `cargo test dump_mock_hsr -- --ignored`: Star Rail preview data.
    #[test]
    #[ignore]
    fn dump_mock_hsr() {
        let cat = banners::Banners::bundled();
        let mut seed: u64 = 99;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let id_of = |n: &str| cat.hsr.items.get(&hsr::norm(n)).map(|x| x.0.clone()).unwrap_or_default();
        let std5 = ["Himeko", "Welt", "Bronya", "Gepard", "Clara", "Yanqing", "Bailu"];
        let std_lc = ["Night on the Milky Way", "In the Name of the World", "Something Irreplaceable", "But the Battle Isn't Over", "Sleep Like the Dead", "Time Waits for No One", "Moment of Victory"];
        let start = chrono::NaiveDate::from_ymd_opt(2025, 4, 1).unwrap().and_hms_opt(20, 0, 0).unwrap();
        let mut out: Vec<data::Wish> = vec![];
        let mut id: u64 = 1_740_000_000_000_000_000;
        for (gt, n, pool, end_pity) in [("11", 560usize, "character", 81u32), ("12", 150, "weapon", 12), ("1", 160, "standard", 33)] {
            let r5 = stats::rates5(Game::Hsr, pool);
            let (mut c5, mut c4, mut lost) = (0u32, 0u32, false);
            for i in 0..n {
                let t = start + chrono::Duration::minutes((i as i64) * (590 * 24 * 60 / n as i64));
                let time = t.format("%Y-%m-%d %H:%M:%S").to_string();
                c5 += 1;
                c4 += 1;
                let (name, rank, ty) = if rnd() < stats::p_at(r5, c5) {
                    c5 = 0;
                    let list = if pool == "weapon" { &cat.hsr.five_cones } else { &cat.hsr.five_chars };
                    let day = &time[..10];
                    let feat: Vec<String> = list.iter().filter(|w| w.start.as_str() <= day && day <= w.end.as_str()).map(|w| w.name.clone()).collect();
                    let win = lost || rnd() < if pool == "weapon" { 0.75 } else { 0.5 };
                    let name = if pool == "standard" || feat.is_empty() || !win {
                        lost = pool != "standard" && !feat.is_empty();
                        if pool == "weapon" { std_lc[(rnd() * 7.0) as usize].to_string() } else { std5[(rnd() * 7.0) as usize].to_string() }
                    } else {
                        lost = false;
                        feat[(rnd() * feat.len() as f64) as usize].clone()
                    };
                    (name, 5u8, if pool == "weapon" { "Light Cone" } else { "Character" })
                } else if c4 >= 10 || rnd() < 0.051 {
                    c4 = 0;
                    (["Pela", "Tingyun", "Gallagher", "Dan Heng", "Post-Op Conversation"][(rnd() * 5.0) as usize].to_string(), 4, "Character")
                } else {
                    (["Arrows", "Cornucopia", "Collapsing Sky", "Amber"][(rnd() * 4.0) as usize].to_string(), 3, "Light Cone")
                };
                id += 1;
                let item_id = id_of(&name);
                out.push(data::Wish { id: id.to_string(), gacha_type: gt.into(), time, name, item_type: ty.into(), rank, item_id });
            }
            let last = out.last().map(|w| w.time.clone()).unwrap_or_default();
            for k in c5..end_pity {
                id += 1;
                let four = (end_pity - k) % 9 == 3;
                let name = if four { "Pela" } else { "Arrows" };
                out.push(data::Wish { id: id.to_string(), gacha_type: gt.into(), time: last.clone(), name: name.into(), item_type: if four { "Character".into() } else { "Light Cone".into() }, rank: if four { 4 } else { 3 }, item_id: id_of(name) });
            }
        }
        out.sort_by(|a, b| a.time.cmp(&b.time).then(a.order().cmp(&b.order())));
        let uid = "702345678";
        let s = stats::summary(Game::Hsr, &out, &cat, uid);
        let v = json!({
            "game": "hsr", "games": ["genshin", "hsr"],
            "accounts": [uid], "uid": uid, "stats": s,
            "active": cat.active(Game::Hsr, 1),
            "status": { "link": "ok", "message": "", "updated": chrono::Utc::now().timestamp() - 300, "working": false },
            "notify": true, "seen_help": true, "game_dir": ""
        });
        std::fs::write("/home/claude/gwc/mock-view-hsr.json", serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
}
