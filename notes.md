# Utility Vest — port notes

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-review

## Class, tier, approach

- **Class `content`, tier `machines`** (ledger row; `horizon/docs/modpacks.md`): the product is a
  wearable item with a container capability and hotbar behaviour, so the shape is content
  (items/recipes/tags) plus native logic (item container + commands). No library/add-on parent:
  it is a core mechanics mod (Curios is a required *library* dependency, handled below).
- First publication **0.1.0**, `name = "Utility Vest"` (the id stays the machine key `utilityvest`).
- Approach: carry the item content from the jar; re-implement the container and hotbar rules in
  a pure, unit-tested `src/runtime.rs`; bind them to the game in `src/support.rs`; prove every
  family in game through `horizon-ctl`, with a control and a `SELFTEST` that ends `EXIT:1`.

## Licence

LGPL-3.0-only, read from the jar's `META-INF/neoforge.mods.toml` (`license = "LGPL3.0"`) and the
upstream repo. Rank 1 copyleft: publishable under the same licence with the complete
corresponding source, which ships inside the package (`src/`, `tools/gen_content.py`, `LICENSE`,
`LICENSE-NOTICE.md`). Gate checks before dispatch: `dispatch-gate.py` exit 0 (not on `main`; a
first publication), `precheck-mod.py` exit 0 (LGPL rank 1), `licence-gate.py` exit 0.

## Jar inventory (measured: 132 entries, 223,694 bytes)

| Family | In the jar | Port treatment (0.1.0) |
|---|---|---|
| 5 vest items + models + textures | yes | carried (LGPL assets) |
| 5 shaped/upgrade recipes + 5 advancements | yes | carried as plain shaped recipes (deviation) |
| `en_us`/`ja_jp`/`pt_br` lang | yes | carried |
| `utilityvest:utility_vest` tag | yes | carried |
| `curios:body` tag / Curios slot | yes | not carried; vanilla body slot via `set_inventory_slot` |
| Container (filters + storage, maxBanks*9 slots) | yes | one item container per tier; saved banks double as filters |
| `saveHotbar`/`loadHotbar`/`swap`/`collectItems` | yes | `save`/`load`/`restock` + `store`/`take` commands; swap deferred |
| `use` -> inventory/filter menus | yes | not in 0.1.0 (menu GUI needs a rendered screen) |
| Keybinds (open gui, load, save, restock, radial) | yes | `/utilityvest` commands (client→server gap, recorded) |
| Radial menu / filter menu / 4 payloads | yes | not ported (client UI, no client→server action helper) |
| Client config (radial cosmetics) | yes | not ported (no server behaviour) |
| `vest_id`/`last_open_row` components | yes | not ported (an upgrade loses its id — deviation) |

## Gate (Step 0, before any code)

The brief's gate: *a player-equipment or inventory-capability surface; stop `blocked(<symbol>)`
if the pin cannot read it.* **The gate passed** on `0a875da` — evidence:

- `surface.json`/`api-source`: `handle:PlayerRef::inventory_slots`, `handle:PlayerRef::
  set_inventory_slot` (numbers 0..35 main, 36..42 equipment; "a worn slot takes any item") —
  the **vanilla body equipment slot 41** is readable and writable for a player.
- `handle:ItemPlace::Nested`, `handle:ItemPlace::stack|insert|extract|exchange`, `ITEM_ITEMS`
  (`item_place.rs:333`) and `Registrar::item_container` + `ItemContainerProvider` — the vest's
  **item container capability** is registerable and reachable where the stack lives.
- `trait:PlaceProvider` + `ItemsRegistrar::place_provider` ("as an accessory mod exposes its
  slots") corroborate that a mod-defined body place exists if the equipment slot did not.
- `curios` has **0** hits in `surface.json`; `library-review.md:32` records Curios as
  `needs-port — no slots API in tree`. So Curios itself is absent — the port uses the vanilla
  equipment slot and the item-container surface, not a Curios emulation.

Verbatim measurements are in `FINDINGS.md` and the gate logs under
`ports/_shared/evidence/utilityvest/` (grep commands and counts).

## TDD red, then green

`src/runtime.rs` was written first with stubs; `cargo test --offline` reported
**`test result: FAILED. 1 passed; 8 failed`** with real assertion failures (e.g.
`left: None right: Some(9)` for `slots_for`), captured in `/tmp/opencode/utilityvest-red.log` and
the evidence dir. After implementation: **`test result: ok. 10 passed; 0 failed`** (the nine
rules plus the engine's `install_parts` acceptance), `cargo clippy --all-targets -- -D warnings`
clean, `cargo fmt --check` clean (`sdk-test.log`, `sdk-clippy.log`, `sdk-fmt.log`).

## Coverage (parity rule)

| Family | Status | In-game assertion |
|---|---|---|
| 5 vest items, models, textures | tested | `ctl give` each id, read back with `ctl inventory`; unknown id refused |
| 5 shaped recipes | tested | craft in game, assert output id + count, ingredients consumed |
| body equipment slot (wear/remove) | tested | `/utilityvest wear` -> slot 41 holds vest; `remove` clears it |
| item container (9/18/... slots) | tested | `store`/`take` move a parsed count into/out of the vest slot |
| save/load hotbar banks | tested | `save 1` writes 9 slots; `load 1` restores the parsed stacks |
| restock (filter matching) | tested | after saving stone, `restock` moves only stone; control leaves dirt |
| Curios slot / keybinds / radial / filter GUI | not ported | pin gap: client-only key hooks, no Curios |
| `swap` quick-select | not ported | needs a worn vest + radial GUI (next slice) |
| `vest_id` component / upgrade NBT | not ported | plain shaped recipes (deviation above) |

## Verification (0.1.0)

| gate | evidence | result |
|---|---|---|
| unit tests (TDD) | `evidence/utilityvest/sdk-test.log` | `test result: ok. 10 passed; 0 failed` (+ 9-test red in `red-first.log`) |
| clippy `-D warnings` | `evidence/utilityvest/sdk-clippy.log` | 0 warnings |
| `cargo fmt --check` | run in the same gated step | clean |
| `cargo horizon-mod build` | `evidence/utilityvest/build.log` | exit 0, `utilityvest.hmod` at the port root |
| `cargo horizon-mod check` | `evidence/utilityvest/check.log` | `44691 bytes, SHA-256 3bf6bcab…3d36b`, 43 files, `EXIT:0` |
| `fits --build nightly` | `evidence/utilityvest/fits.log` | `f1bb66f  fits`, `EXIT:0` |
| `fits --build 0b14f8a` | `evidence/utilityvest/fits-other.log` | `0b14f8a  fits`, `EXIT:0` |
| `launcher-game.sh run` default | `evidence/utilityvest/e2e-default.log` | `PASS …`, `EXIT:0` |
| `CONTROL=1` (plain stick) | `evidence/utilityvest/e2e-control.log` | `PASS: control left 16 stone and refused the action`, `EXIT:0` |
| `SELFTEST=1` (inverted restock) | `evidence/utilityvest/e2e-selftest.log` | `FAIL: SELFTEST: restock moved 24 -> 0`, `EXIT:1` |
| `check-e2e-logs.py utilityvest --strict` | — | `logs=3 ok=3 missing-exit=0 bad-exit=0` |
| `verify-sdk-package.py utilityvest` | — | **1/1** |
| `check-skills.py --notes ports/utilityvest --strict` | — | PASS |

Delivered `.hmod`: `ports/utilityvest/utilityvest.hmod`, **44,691 bytes**, SHA-256
`3bf6bcab49cc88d2dbedd4864f51df219da47618b6723be7d7e33a5272a3d36b`, 43 entries (no packed
`Cargo.lock`; `src/runtime.rs` byte-identical to the tree).


## Gaps and deviations

1. **Curios body slot** -> vanilla equipment slot 41 (`set_inventory_slot`) — the pin has no
   Curios registry; documented above and proven in game.
2. **Keybinds** -> `/utilityvest` commands — the key hooks are client-side listen only.
3. **One container, not two** — saved banks double as the restock filter set.
4. **`upgrade` recipe serializer** -> plain shaped recipes; the vest id/contents are not copied
   on tier upgrade.
5. **Radial/filter GUI, payloads, client config, `swap`** — not ported in 0.1.0.

## Next slice

**Family:** the `swap` quick-select (`UVVestCapability.swap` in `FINDINGS.md`) — the product's
headline "quickly swap hotbars via a hotkey" behaviour, still absent from the coverage table.

**Exact gap line:** coverage table row `swap quick-select | not ported | needs a worn vest + radial
GUI (next slice)`, and deviation #5 (`Radial/filter GUI, payloads, client config, swap`).

**Why it fits the tier:** `machines` — it is the same item-container capability plus the body
equipment slot already proven in 0.1.0; no new hook point or registry surface is needed. The
pin's `PlayerRef::set_selected_slot` plus `ItemPlace::Nested::{stack,exchange}` express the swap,
and `native`/`input_key` is a listen-only client notice, so the server action routes through
`/utilityvest swap <slot>` as the other families do.

**In-game assertion that proves it:** with the vest worn (body slot 41) and a diamond pickaxe in
the main hand, `ctl chat "/utilityvest swap 0"` must put the pickaxe into vest container slot 0
and the stack that was in slot 0 into the main hand; read both with `ctl inventory` and
`ctl state .held`. The counterfactual holds a plain stick and asserts the command returns
`hold a vest` and neither stack moves; `SELFTEST=1` inverts the expected hand item and must end
`EXIT:1`. The next slice also registers the pure `swap_index` rule in `runtime.rs` (it already
has a passing TDD test but is not yet called from the glue).
