# Utility Vest for Horizon

A port of Satherov's [Utility Vest](https://github.com/Satherov/Utility-Vest) (LGPL-3.0-only)
for Horizon (Minecraft Java 26.3, mod API 0.2). Five wearable vest tiers act as portable
nine-slot-bank item containers, with hotbar save/load and restock.

This is slice 1 (`0.1.0`). See `notes.md` for coverage, evidence and the recorded gaps.

## What is in the package

- `utilityvest:{leather,iron,gold,diamond,netherite}_utility_vest` — items, models, textures,
  five shaped recipes, five advancements, three lang files and the `utility_vest` tag.
- Five item containers (9/18/27/36/45 slots), registered from the pure tier rules in
  `src/runtime.rs`.
- `/utilityvest wear|remove|store <slot>|take <slot>|save <bank>|load <bank>|restock` — the
  server-side actions. Upstream's keybinds are client-side listen hooks at this pin, so the
  dispatcher is the testable route.

## Licence and source

LGPL-3.0-only. The complete corresponding source (the three `src/*.rs` files and
`tools/gen_content.py`) travels with the package; `LICENSE` carries the full LGPL v3 / GPL v3
text. Original work © Satherov; port © AdLf0208.

## Build

`cargo horizon-mod build` from the package root produces `utilityvest.hmod`. `tools/gen_content.py`
regenerates the content tree from the cached jar (`--check` verifies it). Builds go through the
campaign's `ports/tools/build-slot.sh`.
