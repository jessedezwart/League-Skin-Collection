# Skin Collection
[![Build](https://github.com/jessedezwart/skin-collection/actions/workflows/build.yml/badge.svg)](https://github.com/jessedezwart/skin-collection/actions/workflows/build.yml)

**This is slop!**

Windows desktop app (Tauri v2 + Svelte 5) that connects to the running League of Legends client, fetches every champion skin your account owns, and shows them in a gallery with their RP value and a total collection value.

## How it works

- **LCU connection** — the Rust backend reads the client's `lockfile` (install dir resolved from Riot's product-settings metadata, falling back to the default path) for the port and password, then talks to `https://127.0.0.1:{port}` with basic auth. The lockfile approach works even when the client runs elevated, which breaks the usual read-the-process-args method.
- **Owned skins** — `GET /lol-champions/v1/inventories/{summonerId}/skins-minimal`, filtered to owned, non-base skins. The inventory lists most skins twice — once under the real id, once as a clone offset by 60,000,000 on a mirrored champion id (Panda Annie is both `1008`/champ `1` and `60001008`/champ `60001`) — so ids are normalized back and deduped, which is what makes the count match the client's own "skins owned" figure. The handful that exist *only* in clone form are the pre-VGU `Classic <champion>` base art; they're counted but left unpriced, since they were never sold for RP.
- **Values** — live RP prices from `GET /lol-store/v1/catalog?inventoryType=["CHAMPION_SKIN"]`. Skins missing from the store (legacy/vaulted) fall back to a rarity-based estimate from CommunityDragon metadata, shown with a `~` prefix. Mythic/Exalted-tier content isn't RP-priced and shows as `—`.
- **EUR/USD value** — RP is converted to money using the base rate of the smallest RP package (€/$4.99 = 575 RP, no bonus RP; NA and EUW price tiers use identical numbers). Larger packages include bonus RP, so the real cost of a collection is lower — the app shows a disclaimer for this. Currency is toggleable (EUR ⇄ USD) and remembered.
- **Availability dates** — also from the store catalog, which carries `releaseDate` for far more skins than it prices (~74% of a typical collection). Skins pulled from the store entirely have no date and sort last. It's called *availability* rather than release because it's the date the item went **on sale**, not when the skin shipped, and Riot overwrites it when a vaulted skin is re-listed — the 2026 "Classic"-mode re-release stamped a batch of 2010-era skins (Riot Kayle, Zombie Ryze, …) with a 2026 date. The client exposes no original-release date anywhere, so there's nothing to correct it against. You sort by one key (value, name, champion, availability); newest-first is always the tiebreaker, so equal-value or same-champion runs read most recent first.
- **Images** — loading-screen and splash art from Data Dragon, downloaded lazily and cached on disk under the app data dir (`image-cache/`), so repeat launches don't re-download. Data Dragon doesn't carry the `Classic <champion>` skins (their art lives under a `Jade_*` character in the game-data tree), so those fall back to CommunityDragon's mirror of `/lol-game-data/assets/`, using the paths from its `skins.json`.

## Development

Requirements: Rust (MSVC toolchain + VS Build Tools with C++ workload), Node.js.

```sh
npm install
npm run tauri dev
```

The app polls for the League client every few seconds — start the client and log in, and the collection loads automatically.

## Build

```sh
npm run tauri build
```
