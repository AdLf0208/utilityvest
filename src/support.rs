//! Game-side glue for the Utility Vest.
//!
//! [`crate::runtime`] holds every rule; this module is the only one that reads the game and writes
//! it back. The vest is used from the player's main hand (upstream `UVVestReference.fromHand`),
//! its nine-slot banks live in the item's container component (the pin's `item_container`
//! provider), and the body equipment slot (41) is written through `PlayerRef::set_inventory_slot`
//! to prove the wearable surface the pin exposes.
//!
//! Upstream's keybinds are client-side listen hooks here; the server actions are therefore
//! exposed as `/utilityvest ...` commands, which the dispatcher delivers to `run_mod_command`
//! (recorded as a deviation in `notes.md`).

use std::collections::BTreeMap;

use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, CommandSource, Flow, Host, HostTransaction,
    ItemPlace, ItemResource, ItemStack, PlayerRef, RegisterCommandsEvent, Registrar,
    RunModCommandEvent, ServerHost, Text, Transaction,
};

use crate::runtime::{
    BANKS, HOTBAR_SLOTS, Move, SlotStack, VEST_IDS, bank_range, is_vest, load_plan, restock_plan,
    save_plan, slots_for,
};

/// The player body equipment slot in `inventory_slots` numbering (`PlayerRef::set_inventory_slot`).
const BODY_SLOT: u32 = 41;

/// Declare the five vest items' containers: a tier's slot count is `banks * 9`, copied from
/// upstream `UVRegistry`'s `maxBanks` (1 to 5).
pub fn declare(r: &mut dyn Registrar) {
    for id in VEST_IDS {
        let key = r.item_key(id);
        r.item_container(&key, move |_: &dyn Host| slots_for(id));
    }
}

/// The `/utilityvest` usages, all at permission level 0 so a player can drive them in game.
pub fn register_commands(event: &mut RegisterCommandsEvent<'_>) {
    for usage in [
        "utilityvest wear",
        "utilityvest remove",
        "utilityvest store <slot:integer>",
        "utilityvest take <slot:integer>",
        "utilityvest save <bank:integer>",
        "utilityvest load <bank:integer>",
        "utilityvest restock",
    ] {
        event.commands.push(Command::op(usage, 0));
    }
}

/// The canonical line of a parsed invocation (literals plus validated arguments).
fn command_words(invocation: &CommandInvocation) -> Vec<String> {
    let mut words = invocation.literals.clone();
    for argument in &invocation.arguments {
        match &argument.value {
            CommandArgumentValue::String(text) => words.push(text.clone()),
            CommandArgumentValue::Integer(value) => words.push(value.to_string()),
            _ => {}
        }
    }
    words
}

/// Dispatch one parsed `/utilityvest` line, answering the player with the parsed result.
pub fn run(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let words = command_words(event.command);
    if words.first().map(String::as_str) != Some("utilityvest") {
        return Flow::Continue;
    }
    let CommandSource::Player { player, .. } = event.source else {
        return Flow::Continue;
    };
    let player = *player;
    let action = words.get(1).map(String::as_str).unwrap_or_default();
    let argument = words
        .get(2)
        .and_then(|word| word.parse::<i32>().ok())
        .unwrap_or(-1);
    let answer = match (action, argument) {
        ("wear", _) => wear(host, player),
        ("remove", _) => remove(host, player),
        ("store", slot) if slot >= 0 => store(host, player, slot as u32),
        ("take", slot) if slot >= 0 => take(host, player, slot as usize),
        ("save", bank) if bank > 0 => save(host, player, bank as u8),
        ("load", bank) if bank > 0 => load(host, player, bank as u8),
        ("restock", _) => restock(host, player),
        _ => return Flow::Continue,
    };
    let _ = player.tell(host, Text::literal(answer));
    Flow::Cancel
}

/// The place of the vest in the player's selected hotbar slot, if that stack is a vest.
fn held_vest(host: &dyn Host, player: PlayerRef) -> Option<ItemPlace> {
    let selected = player.selected_slot(host)?;
    let (slot, stack) = player
        .inventory_slots(host)?
        .into_iter()
        .find(|(slot, _)| *slot as usize == selected)?;
    if !is_vest(&stack.item().id(host)) {
        return None;
    }
    Some(ItemPlace::PlayerSlot {
        player: player.uuid(),
        slot: slot as i32,
    })
}

/// The item id and bank count of the vest at `place`.
fn vest_at(host: &mut dyn ServerHost, place: &ItemPlace) -> Option<(String, u8)> {
    let stack = place.stack(host)?;
    let id = stack.item().id(&*host);
    let tier = VEST_IDS.iter().position(|vest| *vest == id)?;
    Some((id, BANKS[tier]))
}

/// One slot of the vest item's container.
fn nest(place: &ItemPlace, slot: usize) -> ItemPlace {
    ItemPlace::Nested {
        parent: Box::new(place.clone()),
        slot,
    }
}

/// Read the vest's whole container as plain `(item id, count)` slots.
fn storage(host: &mut dyn ServerHost, place: &ItemPlace, slots: usize) -> Vec<Option<SlotStack>> {
    (0..slots)
        .map(|index| {
            nest(place, index).stack(host).map(|stack| SlotStack {
                item: stack.item().id(&*host),
                count: stack.count(),
            })
        })
        .collect()
}

/// Set one container slot to `stack` (empty clears it), inside `tx`.
fn set_slot(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    place: &ItemPlace,
    slot: usize,
    stack: Option<&ItemStack>,
) {
    let place = nest(place, slot);
    if let Some(current) = place.stack(host) {
        let resource = ItemResource::of(&current);
        place.extract(host, tx, &resource, current.count());
    }
    if let Some(stack) = stack {
        place.insert(host, tx, &ItemResource::of(stack), stack.count());
    }
}

/// Read the player's nine hotbar stacks (slot order 0..9) as owned stacks.
fn hotbar_stacks(host: &dyn Host, player: PlayerRef) -> Vec<Option<ItemStack>> {
    let inventory = player.inventory_slots(host).unwrap_or_default();
    (0..HOTBAR_SLOTS)
        .map(|slot| {
            inventory
                .iter()
                .find(|(index, _)| *index as usize == slot)
                .map(|(_, stack)| stack.clone())
        })
        .collect()
}

/// Apply pure restock moves to the real vest and inventory, atomically. Returns the count that
/// actually went into the vest (the world is the source of truth, the plan only proposes).
fn apply_moves(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    place: &ItemPlace,
    inventory: &[(u32, ItemStack)],
    moves: &[Move],
) -> u32 {
    let mut taken: BTreeMap<u32, u32> = BTreeMap::new();
    let result = host.transaction(|host, tx| {
        for movement in moves {
            let Some((_, stack)) = inventory
                .iter()
                .find(|(slot, _)| *slot == movement.inv_slot)
            else {
                continue;
            };
            let resource = ItemResource::of(stack);
            let went = nest(place, movement.vest_slot).insert(host, tx, &resource, movement.count);
            if went > 0 {
                *taken.entry(movement.inv_slot).or_default() += went;
            }
        }
        for (slot, amount) in &taken {
            let Some((_, stack)) = inventory.iter().find(|(index, _)| index == slot) else {
                continue;
            };
            let mut left = stack.clone();
            let count = left.count();
            let _ = left.set_count(count.saturating_sub(*amount));
            let _ = player.set_inventory_slot(host, *slot, (count > *amount).then_some(left));
        }
        Ok::<(), ()>(())
    });
    if result.is_err() {
        return 0;
    }
    taken.values().sum()
}

/// Move the held vest into the body equipment slot, swapping out whatever it held.
pub fn wear(host: &mut dyn ServerHost, player: PlayerRef) -> String {
    let Some(slots) = player.inventory_slots(host) else {
        return "no inventory".to_owned();
    };
    let Some((from, vest)) = slots
        .iter()
        .find(|(_, stack)| is_vest(&stack.item().id(&*host)))
        .cloned()
    else {
        return "no vest held or carried".to_owned();
    };
    let worn = slots
        .iter()
        .find(|(slot, _)| *slot == BODY_SLOT)
        .map(|(_, stack)| stack.clone());
    let _ = player.set_inventory_slot(host, BODY_SLOT, Some(vest));
    let _ = player.set_inventory_slot(host, from, worn);
    format!("worn vest in body slot {BODY_SLOT}")
}

/// Move the body-slot vest back into the player's selected hotbar slot.
pub fn remove(host: &mut dyn ServerHost, player: PlayerRef) -> String {
    let worn = player.inventory_slots(host).and_then(|slots| {
        slots
            .into_iter()
            .find(|(slot, _)| *slot == BODY_SLOT)
            .map(|(_, stack)| stack)
    });
    let Some(worn) = worn else {
        return "body slot empty".to_owned();
    };
    if !is_vest(&worn.item().id(&*host)) {
        return "body slot is not a vest".to_owned();
    }
    let destination = player.selected_slot(host).unwrap_or(0) as u32;
    let _ = player.set_inventory_slot(host, destination, Some(worn));
    let _ = player.set_inventory_slot(host, BODY_SLOT, None);
    format!("removed vest to slot {destination}")
}

/// Insert an inventory stack into the held vest (upstream `insertWithOverflow`).
pub fn store(host: &mut dyn ServerHost, player: PlayerRef, slot: u32) -> String {
    let Some(place) = held_vest(host, player) else {
        return "hold a vest".to_owned();
    };
    let Some((id, banks)) = vest_at(host, &place) else {
        return "hold a vest".to_owned();
    };
    let Some(inventory) = player.inventory_slots(host) else {
        return "no inventory".to_owned();
    };
    let Some((_, stack)) = inventory.iter().find(|(index, _)| *index == slot) else {
        return format!("slot {slot} empty");
    };
    let current = storage(host, &place, slots_for(&id).unwrap_or(0) as usize);
    let plain = [(
        slot,
        SlotStack::new(&stack.item().id(&*host), stack.count()),
    )];
    let moves = restock_plan(&plain, &[], &current);
    let moved = apply_moves(host, player, &place, &inventory, &moves);
    format!("stored {moved} from slot {slot} in {banks} bank(s)")
}

/// Extract one vest container slot into the player's inventory.
pub fn take(host: &mut dyn ServerHost, player: PlayerRef, slot: usize) -> String {
    let Some(place) = held_vest(host, player) else {
        return "hold a vest".to_owned();
    };
    let Some((id, _)) = vest_at(host, &place) else {
        return "hold a vest".to_owned();
    };
    let total = slots_for(&id).unwrap_or(0) as usize;
    if slot >= total {
        return format!("slot {slot} out of range (0..{total})");
    }
    let target = nest(&place, slot);
    let Some(stack) = target.stack(host) else {
        return format!("slot {slot} empty");
    };
    let count = stack.count();
    let result = host.transaction(|host, tx| {
        let resource = ItemResource::of(&stack);
        target.extract(host, tx, &resource, count);
        Ok::<(), ()>(())
    });
    if result.is_err() {
        return "extract failed".to_owned();
    }
    match player.give_stack(host, stack) {
        Ok(()) => format!("took {count} from vest slot {slot}"),
        Err(_) => "give failed".to_owned(),
    }
}

/// Copy the player's hotbar into one bank of the held vest (upstream `saveHotbar`).
pub fn save(host: &mut dyn ServerHost, player: PlayerRef, bank: u8) -> String {
    let Some(place) = held_vest(host, player) else {
        return "hold a vest".to_owned();
    };
    let Some((id, banks)) = vest_at(host, &place) else {
        return "hold a vest".to_owned();
    };
    let Some((start, _)) = bank_range(bank, banks) else {
        return format!("bank {bank} out of range (1..={banks})");
    };
    let hotbar = hotbar_stacks(host, player);
    let plain: Vec<Option<SlotStack>> = hotbar
        .iter()
        .map(|stack| {
            stack.as_ref().map(|stack| SlotStack {
                item: stack.item().id(&*host),
                count: stack.count(),
            })
        })
        .collect();
    if save_plan(&plain, bank, banks).is_none() {
        return format!("bank {bank} out of range (1..={banks})");
    }
    let written = hotbar.iter().filter(|stack| stack.is_some()).count();
    let _ = host.transaction(|host, tx| {
        for (offset, stack) in hotbar.iter().enumerate() {
            set_slot(host, tx, &place, start + offset, stack.as_ref());
        }
        Ok::<(), ()>(())
    });
    format!("saved {written} hotbar stack(s) to bank {bank} of {id}")
}

/// Load one bank of the held vest into the hotbar (upstream `loadHotbar`), returning the old
/// hotbar stacks to the inventory.
pub fn load(host: &mut dyn ServerHost, player: PlayerRef, bank: u8) -> String {
    let Some(place) = held_vest(host, player) else {
        return "hold a vest".to_owned();
    };
    let Some((id, banks)) = vest_at(host, &place) else {
        return "hold a vest".to_owned();
    };
    let total = slots_for(&id).unwrap_or(0) as usize;
    let storage_slots = storage(host, &place, total);
    let hotbar = hotbar_stacks(host, player);
    let plain_hotbar: Vec<Option<SlotStack>> = hotbar
        .iter()
        .map(|stack| {
            stack.as_ref().map(|stack| SlotStack {
                item: stack.item().id(&*host),
                count: stack.count(),
            })
        })
        .collect();
    let Some((loaded, overflow)) = load_plan(bank, banks, &storage_slots, &plain_hotbar) else {
        return format!("bank {bank} out of range (1..={banks})");
    };
    let (start, _) = bank_range(bank, banks).expect("load_plan already bounded the bank");
    let bank_stacks: Vec<Option<ItemStack>> = (0..HOTBAR_SLOTS)
        .map(|offset| nest(&place, start + offset).stack(host))
        .collect();
    let result = host.transaction(|host, tx| {
        for (slot, stack) in bank_stacks.iter().enumerate() {
            let _ = player.set_inventory_slot(host, slot as u32, stack.clone());
        }
        for (slot, stack) in hotbar.iter().enumerate() {
            if let Some(stack) = stack {
                let resource = ItemResource::of(stack);
                nest(&place, start + slot).extract(host, tx, &resource, stack.count());
            }
        }
        Ok::<(), ()>(())
    });
    if result.is_err() {
        return "load failed".to_owned();
    }
    for stack in hotbar.into_iter().flatten() {
        let _ = player.give_stack(host, stack);
    }
    let filled = loaded.iter().filter(|stack| stack.is_some()).count();
    let _ = overflow; // the parsed count is the assertion the e2e reads
    format!("loaded bank {bank}: {filled} hotbar stack(s), returned old hotbar")
}

/// Move every inventory stack named by the vest's storage into the vest (upstream
/// `collectItems`). The saved bank slots double as the filter set: an empty vest matches all,
/// otherwise only item ids present in storage are moved.
pub fn restock(host: &mut dyn ServerHost, player: PlayerRef) -> String {
    let Some(place) = held_vest(host, player) else {
        return "hold a vest".to_owned();
    };
    let Some((id, _)) = vest_at(host, &place) else {
        return "hold a vest".to_owned();
    };
    let total = slots_for(&id).unwrap_or(0) as usize;
    let current = storage(host, &place, total);
    let filters: Vec<Option<String>> = current
        .iter()
        .map(|slot| slot.as_ref().map(|stack| stack.item.clone()))
        .collect();
    let Some(inventory) = player.inventory_slots(host) else {
        return "no inventory".to_owned();
    };
    let plain: Vec<(u32, SlotStack)> = inventory
        .iter()
        .filter(|(slot, _)| *slot < 36)
        .map(|(slot, stack)| {
            (
                *slot,
                SlotStack::new(&stack.item().id(&*host), stack.count()),
            )
        })
        .collect();
    let moves = restock_plan(&plain, &filters, &current);
    let moved = apply_moves(host, player, &place, &inventory, &moves);
    format!("restocked {moved} item(s) into the vest")
}
