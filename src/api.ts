import { invoke } from "@tauri-apps/api/core";

export type Five = { name: string; id: string; item_type: string; time: string; pity: number; result: "" | "won" | "lost" | "guaranteed"; early: boolean };
export type Recent = { name: string; id: string; item_type: string; rank: number; time: string };
export type Pool = {
  pool: "character" | "weapon" | "chronicled" | "standard" | "beginner";
  total: number; pity5: number; pity4: number; soft: number; hard: number; hard4: number;
  zone: "normal" | "soft" | "hard"; guaranteed: boolean | null;
  chance_next: number; chance10: number; expected_left: number;
  fives: Five[]; fours: number; avg_pity: number; expected_avg: number;
  won: number; lost: number; early: number; expected_early: number; recent: Recent[];
};
export type Summary = {
  total: number; primogems: number; fives: number; fours: number;
  won: number; lost: number; weapon_won: number; weapon_lost: number;
  early: number; expected_early: number; avg_pity: number; expected_avg: number; pools: Pool[];
};
export type Active = { kind: "characters" | "weapons" | "chronicled"; name: string; short_name: string; image: number; color: string; version: string; featured: string[]; featured_rare: string[]; ends: number };
export type Status = { link: "none" | "ok" | "expired" | "error"; message: string; updated: number; working: boolean };
export type View = { accounts: string[]; uid: string; stats: Summary | null; active: Active[]; status: Status; notify: boolean; seen_help: boolean; game_dir: string };

export const api = {
  view: () => invoke<View>("get_view"),
  refresh: (link?: string) => invoke<number>("refresh", { link: link ?? null }),
  select: (uid: string) => invoke<void>("select_account", { uid }),
  prefs: (notify: boolean, seenHelp: boolean, gameDir: string) => invoke<void>("set_prefs", { notify, seenHelp, gameDir }),
  exportUigf: (uid: string, path: string) => invoke<number>("export_uigf", { uid, path }),
  importUigf: (path: string) => invoke<number>("import_uigf", { path }),
};

const IMG = "https://paimon.moe/images";
export const charImg = (id: string) => `${IMG}/characters/${id}.png`;
export const weaponImg = (id: string) => `${IMG}/weapons/${id}.png`;
/** paimon.moe ids don't say what they are: weapons on weapon banners, otherwise characters (chronicled lists both). */
export const itemImg = (id: string, type?: string) => (type === "Weapon" ? weaponImg(id) : charImg(id));
export const bannerImg = (a: Active) => `${IMG}/banners/${encodeURIComponent(`${a.name} ${a.image}`)}.png`;

export const pct = (x: number, digits = 0) => `${(x * 100).toFixed(digits)}%`;
export function until(unix: number) {
  const s = unix - Date.now() / 1000;
  if (s <= 0) return "ending now";
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d > 0 ? `${d}d ${h}h left` : h > 0 ? `${h}h ${m}m left` : `${m}m left`;
}
export function ago(unix: number) {
  if (!unix) return "never";
  const s = Date.now() / 1000 - unix;
  if (s < 90) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}
export const POOL_NAME: Record<Pool["pool"], string> = {
  character: "Character Event",
  weapon: "Weapon Event",
  chronicled: "Chronicled Wish",
  standard: "Standard",
  beginner: "Beginners'",
};
