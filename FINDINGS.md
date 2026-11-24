# Utility Vest — upstream findings

Artefact: `ports/_shared/licence-cache/utilityvest-1.3.1.jar` (223,694 bytes, 132 entries).
Source read (public repository, clean-room behaviour analysis only — no Java copied):
`https://github.com/Satherov/Utility-Vest`, branch `1.21.1`, files
`core/UVRegistry.java`, `common/item/UVVestItem.java`, `common/item/UVVestReference.java`,
`common/capabilities/UVVestCapability.java`, `config/UVConfig.java`.

## Licence

`META-INF/neoforge.mods.toml`: `license = "LGPL3.0"`, author Satherov, issue tracker the same
GitHub repo. Rank 1 copyleft — publishable under LGPL-3.0 with the complete corresponding source,
which travels in the package (`src/`, `tools/gen_content.py`, `LICENSE`).

## What the jar contains (measured, not inferred from the name)

- Five items: `utilityvest:{leather,iron,gold,diamond,netherite}_utility_vest`, each a
  `stacksTo(1)` `UVVestItem` with `maxBanks` 1..5 (`UVRegistry`), a model and a 16-32 px texture.
- Five recipes: one `minecraft:crafting_shaped` for the leather vest (`c:leathers`/`c:chests`)
  and four **custom `utilityvest:upgrade`** recipes chaining leather -> iron -> gold -> diamond ->
  netherite with the tier's ingot/gem. The custom serializer copies the old vest's id and
  contents into the upgrade.
- Five recipe advancements, `assets/utilityvest/lang/{en_us,ja_jp,pt_br}.json`, the item tag
  `utilityvest:utility_vest`, a `curios:body` tag and the mod's mixin config.
- Classes: `UVVestCapability` (the container + behaviour), `UVVestItem` (use/filter menus),
  `UVVestReference` (where the vest is found), `UVInventoryMenu`/`UVFilterMenu`/
  `UVFilterMenu`/`RadialMenuScreen` (GUI), `UVKeybindManager` and four network payloads
  (OpenVest, Restock, SaveLoad, SwapTool). Curios is a **required** dependency in `mods.toml`.

## Behaviour extracted (what the port re-implements)

- `UVVestCapability` has two `ComponentItemHandler`s on the stack: `filters` and `storage`,
  each `maxBanks * 9` slots (9/18/27/36/45). A stack's `VEST_ID` UUID identifies its vest.
- `matchFilter(stack)`: a vest in the tag never matches; if every filter slot is empty, every
  item matches; otherwise an item matches when any filter slot holds the same item
  (`ItemStack.isSameItem`).
- `saveHotbar(player, bank)`: copies the nine hotbar stacks into filter slots `bank*9..+9`.
  `loadHotbar(player, bank)`: items in the hotbar that match the filters go into storage (with
  overflow), the rest return to the inventory; the hotbar is cleared and rebuilt from the
  bank's filter templates, pulling the named counts out of storage.
- `swap(tool, idx)`: swaps the held stack with storage slot `idx`, or the first empty slot when
  `idx == -1` (the radial quick-select).
- `collectItems(player)` (restock): every main-inventory/offhand stack matching the filters is
  inserted into storage (same-item slots first, then empty, capped at 64).
- `UVVestReference.findFirstAccessible` checks the **main hand, off hand, then Curios slots** —
  a vest need not be worn for its menus to open. `UVVestItem.use` opens the inventory menu, or
  the filter menu when crouching.
- `UVConfig` is client-only radial-menu cosmetics (scroll inversion, tooltip display, stack-count
  display, remember-row) — no server behaviour.

## Where the port deliberately differs

- **Curios body slot** -> the pin's vanilla body equipment slot (inventory slot 41), written
  through `PlayerRef::set_inventory_slot`. The pin exposes no Curios/accessory registry, but does
  expose the equipment slot directly and the `ItemPlace`/`ItemContainerProvider` surface.
- **Keybinds / radial GUI / filter GUI** -> `/utilityvest ...` commands. At this pin the key
  hooks are client-side listen notifications; the dispatcher is the fail-capable server path.
- **Custom `upgrade` recipe serializer** -> plain shaped recipes with concrete vanilla
  ingredients; the vest's `VEST_ID`/contents are not copied on upgrade (recorded deviation).
- **Two item containers (filters + storage)** -> one item container per tier, with the saved bank
  slots doubling as the filter set for restock.
- Curios tag, `last_open_row`, `vest_id` components and client config are not carried; the
  complete-source obligation is met by shipping `src/` and the generator in the package.
