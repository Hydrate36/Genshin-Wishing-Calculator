# Genshin Wishing Calculator

A small desktop app that tracks your pity on every banner — automatically — for **Genshin Impact** and **Honkai: Star Rail**.

- **Both games in one app:** switch between Genshin Impact and Honkai: Star Rail at the top.
- **Auto-tracking:** open Wish History (Genshin) or Warp → Records (Star Rail) in the game once and the app picks up your whole history, then keeps checking for new wishes while it's open. No copying links, no scripts.
- **Active banners** with the featured 5★ and 4★ and how long they have left.
- **Pity for every banner** — Genshin: Character Event, Weapon Event, Chronicled Wish, Standard; Star Rail: Character Warp, Light Cone Warp (collaboration warps count toward them), Stellar Warp — with soft pity and hard pity marked, and a **notification when you reach soft pity and hard pity**.
- **Your odds:** chance of a 5★ on the next wish and within 10, wishes still to go on average, guaranteed or 50/50.
- **Your luck:** 50/50s won, early 5★s (before soft pity) and average pity, each next to what's expected.
- **5★ history** per banner with pity, won/lost 50/50 and early.
- **Several accounts** (one per UID) and **UIGF import/export** (and SRGF import), so you can move history from or to other trackers.

## How it works

When you open Wish History or Warp Records, the game saves a link in its web cache that can read your wish history for about a day. The app reads that link from your PC, downloads your history from HoYoverse and keeps it on your PC. The link is only kept in memory and never shared. The game's history updates with about an hour of delay.

Genshin banner data comes from [paimon.moe](https://github.com/MadeBaruna/paimon-moe)'s open data; Star Rail banner dates from [samsara](https://github.com/benlei/samsara-web) and icons from [StarRailRes](https://github.com/Mar-7th/StarRailRes). Odds use the community model of the games' rates: 0.6% per pull, rising by 6% per pull from 74, guaranteed at 90; Genshin weapons 0.7%, from 63, guaranteed at 80; Star Rail Light Cones 0.8%, from 66, guaranteed at 80.

## Download

Get the installer from [Releases](https://github.com/Hydrate36/Genshin-Wishing-Calculator/releases). The app updates itself.

## Build

Rust + Tauri 2, React for the window. `npm install`, then `npx tauri build`.

`wish-tracker.exe --check` runs the link search and download without a window and prints a summary.

## Version 1

The old Python/Tkinter calculator is still in this repository's history (tag `v1-python`).

Genshin Impact and Honkai: Star Rail are trademarks of HoYoverse. This is a fan project and isn't affiliated with HoYoverse.

## License

MIT
