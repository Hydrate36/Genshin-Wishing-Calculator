# Genshin Wishing Calculator

A small desktop app that tracks your Genshin Impact pity on every banner — automatically.

- **Auto-tracking:** open Wish History in the game once and the app picks up your whole history, then keeps checking for new wishes while it's open. No copying links, no scripts.
- **Active banners** with the featured 5★ and 4★ and how long they have left.
- **Pity for every banner** — Character Event, Weapon Event, Chronicled Wish and Standard — with soft pity and hard pity marked, and a **notification when you reach soft pity and hard pity**.
- **Your odds:** chance of a 5★ on the next wish and within 10, wishes still to go on average, guaranteed or 50/50.
- **Your luck:** 50/50s won, early 5★s (before soft pity) and average pity, each next to what's expected.
- **5★ history** per banner with pity, won/lost 50/50 and early.
- **Several accounts** (one per UID) and **UIGF import/export**, so you can move history from or to other trackers.

## How it works

When you open Wish History, the game saves a link in its web cache that can read your wish history for about a day. The app reads that link from your PC, downloads your history from HoYoverse and keeps it on your PC. The link is only kept in memory and never shared. The game's history updates with about an hour of delay.

Banner data comes from [paimon.moe](https://github.com/MadeBaruna/paimon-moe)'s open data. Odds use the community model of the game's rates: 0.6% per wish, rising by 6% per wish from wish 74, guaranteed at 90; weapons 0.7%, from wish 63, guaranteed at 80.

## Download

Get the installer from [Releases](https://github.com/Hydrate36/Genshin-Wishing-Calculator/releases). The app updates itself.

## Build

Rust + Tauri 2, React for the window. `npm install`, then `npx tauri build`.

`wish-tracker.exe --check` runs the link search and download without a window and prints a summary.

## Version 1

The old Python/Tkinter calculator is still in this repository's history (tag `v1-python`).

Genshin Impact is a trademark of HoYoverse. This is a fan project and isn't affiliated with HoYoverse.

## License

MIT
