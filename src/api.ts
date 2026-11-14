import { invoke } from "@tauri-apps/api/core";

export type Game = "genshin" | "hsr";
export type Item = { name: string; icon: string; alt: string };
export type Five = { name: string; id: string; icon: string; item_type: string; time: string; pity: number; result: "" | "won" | "lost" | "guaranteed"; early: boolean };
export type Recent = { name: string; id: string; icon: string; item_type: string; rank: number; time: string };
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
export type Active = { kind: "characters" | "weapons" | "chronicled"; name: string; art: string; color: string; version: string; featured: Item[]; featured_rare: Item[]; ends: number };
export type Status = { link: "none" | "ok" | "expired" | "error"; message: string; updated: number; working: boolean };
export type View = { game: Game; games: Game[]; accounts: string[]; uid: string; stats: Summary | null; active: Active[]; status: Status; notify: boolean; seen_help: boolean; game_dir: string };

export const api = {
  view: (game?: Game) => invoke<View>("get_view", { game: game ?? null }),
  setGame: (game: Game) => invoke<void>("set_game", { game }),
  refresh: (game: Game, link?: string) => invoke<number>("refresh", { game, link: link ?? null }),
  select: (game: Game, uid: string) => invoke<void>("select_account", { game, uid }),
  prefs: (game: Game, notify: boolean, seenHelp: boolean, gameDir: string) => invoke<void>("set_prefs", { game, notify, seenHelp, gameDir }),
  exportUigf: (game: Game, uid: string, path: string) => invoke<number>("export_uigf", { game, uid, path }),
  importUigf: (path: string) => invoke<number>("import_uigf", { path }),
};

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
/** Everything worded differently per game. */
export const WORDS = {
  genshin: {
    title: "Genshin Impact",
    pull: "wish",
    pulls: "wishes",
    pools: { character: "Character Event", weapon: "Weapon Event", chronicled: "Chronicled Wish", standard: "Standard", beginner: "Beginners'" } as Record<Pool["pool"], string>,
    kinds: { characters: "Character Event", weapons: "Weapon Event", chronicled: "Chronicled Wish" } as Record<Active["kind"], string>,
    weaponFifty: "75/25s won (weapons)",
    open: ["Open Genshin Impact on this PC and log in.", "Press F3 (Wish), then History at the bottom. Any banner is fine."],
    reopen: "open Wish History in the game",
    rates: "Odds use the community model of the game's rates (0.6% base, soft pity from wish 74, 90 max; weapons 0.7%, from 63, 80 max). The game's history updates with about an hour of delay.",
    types: { "301": "Character Event", "302": "Weapon Event", "500": "Chronicled Wish", "200": "Standard", "100": "Beginners'" } as Record<string, string>,
  },
  hsr: {
    title: "Honkai: Star Rail",
    pull: "warp",
    pulls: "warps",
    pools: { character: "Character Warp", weapon: "Light Cone Warp", chronicled: "", standard: "Stellar Warp", beginner: "Departure Warp" } as Record<Pool["pool"], string>,
    kinds: { characters: "Character Event Warp", weapons: "Light Cone Event Warp", chronicled: "" } as Record<Active["kind"], string>,
    weaponFifty: "75/25s won (Light Cones)",
    open: ["Open Honkai: Star Rail on this PC and log in.", "Open Warp (Esc menu or the Warp icon), then Records at the bottom. Any warp is fine."],
    reopen: "open Warp → Records in the game",
    rates: "Odds use the community model of the game's rates (0.6% base, soft pity from warp 74, 90 max; Light Cones 0.8%, from 66, 80 max). Collaboration warps count toward the event warps' pity. The game's records update with about an hour of delay.",
    types: { "11": "Character Warp", "21": "Collab Character Warp", "12": "Light Cone Warp", "22": "Collab Light Cone Warp", "1": "Stellar Warp", "2": "Departure Warp" } as Record<string, string>,
  },
};
