import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Active, ago, api, bannerImg, charImg, Five, itemImg, pct, Pool, POOL_NAME, until, View, weaponImg } from "./api";

type Toast = { text: string; bad?: boolean } | null;

export default function App() {
  const [v, setV] = useState<View | null>(null);
  const [toast, setToast] = useState<Toast>(null);
  const [progress, setProgress] = useState<string>("");
  const [menu, setMenu] = useState(false);
  const [paste, setPaste] = useState(false);
  const [tab, setTab] = useState<Pool["pool"]>("character");
  const [update, setUpdate] = useState<null | (() => void)>(null);

  const load = useCallback(() => api.view().then(setV).catch(() => {}), []);
  const show = (t: Toast, ms = 4000) => { setToast(t); if (t) setTimeout(() => setToast((c) => (c === t ? null : c)), ms); };

  useEffect(() => {
    load();
    const un1 = listen("changed", () => { load(); setProgress(""); });
    const un2 = listen<{ banner: string; count: number }>("progress", (e) => setProgress(`${typeName(e.payload.banner)}: ${e.payload.count} wishes`));
    const tick = setInterval(load, 30000);
    check().then((u) => { if (u) setUpdate(() => async () => { await u.downloadAndInstall(); await relaunch(); }); }).catch(() => {});
    return () => { un1.then((f) => f()); un2.then((f) => f()); clearInterval(tick); };
  }, [load]);

  const refresh = async (link?: string) => {
    try {
      const n = await api.refresh(link);
      show({ text: n ? `${n} new wish${n === 1 ? "" : "es"} added.` : "Up to date." });
    } catch (e) {
      show({ text: String(e), bad: true }, 8000);
    } finally {
      setProgress("");
      load();
    }
  };

  if (!v) return <div className="app"><TitleBar /></div>;
  const s = v.stats;
  const pools = s?.pools.filter((p) => p.pool !== "beginner" || p.total > 0) ?? [];
  const hasWishes = !!s && s.total > 0;

  return (
    <div className="app">
      <TitleBar />
      <header className="top">
        <div className="brand"><Star /> <span>Wishing Calculator</span></div>
        {v.accounts.length > 0 && (
          <select className="field" value={v.uid} onChange={(e) => api.select(e.target.value).then(load)} aria-label="Account">
            {v.accounts.map((u) => <option key={u} value={u}>UID {u}</option>)}
          </select>
        )}
        <div className="grow" />
        <StatusPill v={v} progress={progress} />
        <button className="btn primary" disabled={v.status.working} onClick={() => refresh()}>{v.status.working ? "Updating…" : "Refresh"}</button>
        <div className="menu-wrap">
          <button className="btn icon" aria-label="More" onClick={() => setMenu(!menu)}>⋯</button>
          {menu && (
            <div className="menu" onMouseLeave={() => setMenu(false)}>
              <button onClick={() => { setMenu(false); setPaste(true); }}>Paste a wish link…</button>
              <button onClick={async () => {
                setMenu(false);
                const f = await openDialog({ multiple: false, filters: [{ name: "UIGF wish history", extensions: ["json"] }] });
                if (typeof f === "string") api.importUigf(f).then((n) => { show({ text: `Imported ${n} wishes.` }); load(); }).catch((e) => show({ text: String(e), bad: true }));
              }}>Import from another tracker (UIGF)…</button>
              <button disabled={!hasWishes} onClick={async () => {
                setMenu(false);
                const f = await saveDialog({ defaultPath: `wishes-${v.uid}.json`, filters: [{ name: "UIGF wish history", extensions: ["json"] }] });
                if (f) api.exportUigf(v.uid, f).then((n) => show({ text: `Saved ${n} wishes.` })).catch((e) => show({ text: String(e), bad: true }));
              }}>Export (UIGF)…</button>
              <button onClick={async () => {
                setMenu(false);
                const d = await openDialog({ directory: true, title: "Pick the Genshin Impact game folder" });
                if (typeof d === "string") { await api.prefs(v.notify, v.seen_help, d); load(); refresh(); }
              }}>Pick game folder…</button>
              <label className="menu-check"><input type="checkbox" checked={v.notify} onChange={(e) => api.prefs(e.target.checked, v.seen_help, v.game_dir).then(load)} /> Soft/hard pity notifications</label>
            </div>
          )}
        </div>
      </header>

      <main className="scroll">
        {update && <div className="banner-update"><span>A new version is ready.</span><button className="btn primary" onClick={update}>Update now</button></div>}

        {v.active.length > 0 && (
          <section>
            <h2>Active banners</h2>
            <div className="banners">{v.active.map((a) => <BannerCard key={a.kind + a.name + a.image} a={a} />)}</div>
          </section>
        )}

        {!hasWishes ? <Welcome v={v} onPaste={() => setPaste(true)} /> : (
          <>
            <section>
              <h2>Your pity</h2>
              <div className="pity-grid">{pools.filter((p) => p.pool !== "beginner").map((p) => <PityCard key={p.pool} p={p} />)}</div>
            </section>
            <section>
              <h2>Your luck</h2>
              <Luck s={s!} />
            </section>
            <section>
              <div className="sec-head">
                <h2>5★ history</h2>
                <div className="seg">
                  {pools.map((p) => <button key={p.pool} className={tab === p.pool ? "on" : ""} onClick={() => setTab(p.pool)}>{POOL_NAME[p.pool]} <span className="dim">{p.fives.length}</span></button>)}
                </div>
              </div>
              <History p={pools.find((p) => p.pool === tab) ?? pools[0]} />
            </section>
            <p className="foot">Odds use the community model of the game's rates (0.6% base, soft pity from wish 74, 90 max; weapon 0.7%, from 63, 80 max). The game's history updates with about an hour of delay.</p>
          </>
        )}
      </main>
      {paste && <PasteLink onClose={() => setPaste(false)} onRun={(l) => { setPaste(false); refresh(l); }} />}
      {toast && <div className={"toast" + (toast.bad ? " bad" : "")}>{toast.text}</div>}
    </div>
  );
}

function typeName(t: string) {
  return { "301": "Character Event", "302": "Weapon Event", "500": "Chronicled Wish", "200": "Standard", "100": "Beginners'" }[t] ?? t;
}

function TitleBar() {
  const w = getCurrentWindow();
  return (
    <div className="titlebar" data-tauri-drag-region>
      <span data-tauri-drag-region>Genshin Wishing Calculator</span>
      <div className="wbtns">
        <button aria-label="Minimize" onClick={() => w.minimize()}>–</button>
        <button aria-label="Maximize" onClick={() => w.toggleMaximize()}>□</button>
        <button aria-label="Close" className="x" onClick={() => w.close()}>×</button>
      </div>
    </div>
  );
}

function Star({ size = 18 }: { size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden><path d="M12 2l2.9 6.3 6.9.8-5.1 4.7 1.4 6.8L12 17.2 5.9 20.6l1.4-6.8L2.2 9.1l6.9-.8z" /></svg>;
}

function StatusPill({ v, progress }: { v: View; progress: string }) {
  const st = v.status;
  if (st.working) return <span className="pill busy">{progress || "Checking for new wishes…"}</span>;
  if (st.link === "ok") return <span className="pill ok" title="New wishes are picked up automatically every few minutes.">Auto-tracking · updated {ago(st.updated)}</span>;
  if (st.link === "expired") return <span className="pill warn" title={st.message}>Link expired — open Wish History in the game</span>;
  if (st.link === "error") return <span className="pill warn" title={st.message}>{st.message}</span>;
  return <span className="pill">Waiting for the game — open Wish History once</span>;
}

/** A picture from paimon.moe; chronicled lists mix characters and weapons, so try the other folder if one is missing. */
function ItemPic({ id, type, rank, size = 46 }: { id: string; type?: string; rank?: number; size?: number }) {
  const [src, setSrc] = useState(type === "Weapon" ? weaponImg(id) : charImg(id));
  const [failed, setFailed] = useState(0);
  return (
    <div className={"pic r" + (rank ?? 5)} style={{ width: size, height: size }}>
      {failed < 2 ? <img src={src} alt="" loading="lazy" onError={() => { setFailed((f) => f + 1); setSrc(src.includes("/weapons/") ? charImg(id) : weaponImg(id)); }} /> : <span>{id.slice(0, 1).toUpperCase()}</span>}
    </div>
  );
}

function BannerCard({ a }: { a: Active }) {
  const [art, setArt] = useState(true);
  const fives = a.kind === "weapons" ? a.featured.slice(0, 2) : a.featured;
  const label = a.kind === "characters" ? "Character Event" : a.kind === "weapons" ? "Weapon Event" : "Chronicled Wish";
  return (
    <div className="bcard" style={{ ["--c" as string]: a.color || "#a78bfa" }}>
      {art ? <img className="bart" src={bannerImg(a)} alt="" onError={() => setArt(false)} /> : <div className="bart none" />}
      <div className="binfo">
        <div className="kind">{label} · {a.version}</div>
        <div className="bname">{a.name}</div>
        <div className="ends">{until(a.ends)}</div>
        <div className="feat">
          {fives.slice(0, a.kind === "chronicled" ? 6 : 2).map((id) => <ItemPic key={id} id={id} type={a.kind === "weapons" ? "Weapon" : undefined} rank={5} size={40} />)}
          {a.kind !== "chronicled" && a.featured_rare.slice(0, a.kind === "weapons" ? 5 : 3).map((id) => <ItemPic key={id} id={id} type={a.kind === "weapons" ? "Weapon" : undefined} rank={4} size={34} />)}
        </div>
      </div>
    </div>
  );
}

function PityCard({ p }: { p: Pool }) {
  const fill = Math.min(1, p.pity5 / p.hard);
  const softAt = (p.soft - 1) / p.hard;
  const zoneText = p.zone === "hard" ? "Hard pity — next wish is a 5★" : p.zone === "soft" ? "Soft pity — odds are up" : `Soft pity in ${p.soft - 1 - p.pity5} wishes`;
  const fiftyName = p.pool === "weapon" ? "75/25" : "50/50";
  return (
    <div className={"pcard zone-" + p.zone}>
      <div className="phead">
        <div className="pname">{POOL_NAME[p.pool]}</div>
        {p.guaranteed !== null && p.total > 0 && <span className={"chip " + (p.guaranteed ? "gold" : "")}>{p.guaranteed ? "Guaranteed featured" : fiftyName}</span>}
      </div>
      <div className="pnum"><b>{p.pity5}</b><span>/ {p.hard}</span></div>
      <div className="bar" aria-label={`${p.pity5} of ${p.hard}`}>
        <div className="fill" style={{ width: pct(fill, 1) }} />
        <div className="soft-mark" style={{ left: pct(softAt, 1) }} title={`Soft pity starts at wish ${p.soft}`} />
      </div>
      <div className="zone">{zoneText}</div>
      <div className="pstats">
        <div><span>Next wish</span><b>{pct(p.chance_next, p.chance_next < 0.1 ? 1 : 0)}</b></div>
        <div><span>Within 10</span><b>{pct(p.chance10)}</b></div>
        <div><span>Avg. to go</span><b>~{Math.round(p.expected_left)}</b></div>
        <div><span>4★ pity</span><b>{p.pity4}/{p.hard4}</b></div>
      </div>
    </div>
  );
}

function Compare({ label, mine, expected, detail, good = "high" }: { label: string; mine: number | null; expected: number; detail: string; good?: "high" | "low" }) {
  const better = mine === null ? null : good === "high" ? mine >= expected : mine <= expected;
  return (
    <div className="lcard">
      <div className="lname">{label}</div>
      <div className="lnum"><b className={better === null ? "" : better ? "good" : "bad"}>{mine === null ? "—" : pct(mine)}</b><span>you</span></div>
      <div className="cmp"><div className="me" style={{ width: pct(Math.min(1, mine ?? 0)) }} /><div className="exp" style={{ left: pct(expected) }} /></div>
      <div className="ldetail">{detail} · expected {pct(expected)}</div>
    </div>
  );
}

function Luck({ s }: { s: View["stats"] & {} }) {
  const fifty = s.won + s.lost;
  const w75 = s.weapon_won + s.weapon_lost;
  const nonBeginner = s.pools.filter((p) => p.pool !== "beginner").reduce((a, p) => a + p.fives.length, 0);
  return (
    <div className="luck">
      <Compare label="50/50s won" mine={fifty ? s.won / fifty : null} expected={0.5} detail={`${s.won} won, ${s.lost} lost`} />
      <Compare label="Early 5★s" mine={nonBeginner ? s.early / nonBeginner : null} expected={s.expected_early} detail={`${s.early} of ${nonBeginner} before soft pity`} />
      <Compare label="75/25s won (weapons)" mine={w75 ? s.weapon_won / w75 : null} expected={0.75} detail={`${s.weapon_won} won, ${s.weapon_lost} lost`} />
      <div className="lcard totals">
        <div><span>Average 5★ pity</span><b className={s.avg_pity && s.avg_pity <= s.expected_avg ? "good" : s.avg_pity ? "bad" : ""}>{s.avg_pity ? s.avg_pity.toFixed(1) : "—"}</b><em>expected {s.expected_avg.toFixed(1)}</em></div>
        <div><span>Wishes</span><b>{s.total.toLocaleString()}</b><em>{s.primogems.toLocaleString()} primogems</em></div>
        <div><span>5★ / 4★</span><b>{s.fives} / {s.fours}</b><em>&nbsp;</em></div>
      </div>
    </div>
  );
}

function History({ p }: { p: Pool }) {
  const list = useMemo(() => [...p.fives].reverse(), [p]);
  if (!list.length) return <div className="empty">No 5★ on this banner yet — {p.pity5} wishes in.</div>;
  return (
    <div className="hist">
      {p.pity5 > 0 && (
        <div className="hrow current">
          <div className="pic r0" style={{ width: 40, height: 40 }}><span>?</span></div>
          <div className="hname">Next 5★<span className="dim">in progress</span></div>
          <PityBar pity={p.pity5} p={p} />
          <span />
        </div>
      )}
      {list.map((f, i) => <HistRow key={f.time + f.name + i} f={f} p={p} />)}
    </div>
  );
}

function PityBar({ pity, p }: { pity: number; p: Pool }) {
  const z = pity >= p.soft ? "soft" : "normal";
  return (
    <div className="hbar"><div className={"hfill " + z} style={{ width: pct(Math.min(1, pity / p.hard), 1) }} /><b>{pity}</b></div>
  );
}

function HistRow({ f, p }: { f: Five; p: Pool }) {
  const tag = f.result === "won" ? ["Won 50/50", "won"] : f.result === "lost" ? ["Lost 50/50", "lost"] : f.result === "guaranteed" ? ["Guaranteed", "gold"] : null;
  const label = tag && p.pool === "weapon" ? [tag[0].replace("50/50", "75/25"), tag[1]] : tag;
  return (
    <div className="hrow">
      <ItemPic id={f.id} type={f.item_type} rank={5} size={40} />
      <div className="hname">{f.name}<span className="dim">{f.time.slice(0, 10)}</span></div>
      <PityBar pity={f.pity} p={p} />
      <div className="tags">
        {f.early && <span className="chip early">Early</span>}
        {label && <span className={"chip " + label[1]}>{label[0]}</span>}
      </div>
    </div>
  );
}

function Welcome({ v, onPaste }: { v: View; onPaste: () => void }) {
  return (
    <section className="welcome">
      <h2>Let's find your wishes</h2>
      <ol>
        <li><b>Open Genshin Impact</b> on this PC and log in.</li>
        <li>Press <b>F3</b> (Wish), then <b>History</b> at the bottom. Any banner is fine.</li>
        <li>Come back here. The app finds the wish link the game just saved, downloads your whole history and keeps checking for new wishes while it's open.</li>
      </ol>
      <p className="dim">The game's link works for about a day. When it runs out, open Wish History in the game again — no need to copy anything.{v.status.link === "error" && v.status.message ? ` Last try: ${v.status.message}` : ""}</p>
      <div className="row">
        <button className="btn" onClick={onPaste}>Paste a wish link instead</button>
      </div>
    </section>
  );
}

function PasteLink({ onClose, onRun }: { onClose: () => void; onRun: (l: string) => void }) {
  const [l, setL] = useState("");
  const ok = l.startsWith("https://") && l.includes("authkey=");
  return (
    <div className="modal-bg" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="modal">
        <h3>Paste a wish link</h3>
        <p className="dim">For players on another PC or phone: a wish-history link that contains <code>authkey=</code>. It's only used to download your history and is never saved.</p>
        <input className="field" autoFocus placeholder="https://…authkey=…" value={l} onChange={(e) => setL(e.target.value.trim())} />
        <div className="row end">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" disabled={!ok} onClick={() => onRun(l)}>Load wishes</button>
        </div>
      </div>
    </div>
  );
}
