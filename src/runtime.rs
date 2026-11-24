//! Pure rules for the Utility Vest port (no `mod_api` types, no world access).
//!
//! Upstream (`Satherov/Utility-Vest`, `UVVestCapability.java`) gives each vest tier `banks`
//! hotbars of nine slots, stores both a filter set and a storage set on the item, and defines
//! four actions: `saveHotbar`, `loadHotbar`, `swap` and `collectItems` (restock). This module
//! holds the decisions of those actions; [`crate::support`] does the reading and writing.
//!
//! A vest is never a valid filter (upstream `matchFilter` refuses `utilityvest:utility_vest`),
//! an empty filter set matches everything, and otherwise an item matches when any filter names
//! that item id (`ItemStack.isSameItem`). Stacks stack to 64.

/// The five vest item ids, leather -> netherite, one per tier.
pub const VEST_IDS: [&str; 5] = [
    "utilityvest:leather_utility_vest",
    "utilityvest:iron_utility_vest",
    "utilityvest:gold_utility_vest",
    "utilityvest:diamond_utility_vest",
    "utilityvest:netherite_utility_vest",
];

/// The number of nine-slot banks each tier holds (upstream `UVRegistry`'s `maxBanks`).
pub const BANKS: [u8; 5] = [1, 2, 3, 4, 5];

/// Slots in one bank (a hotbar).
pub const HOTBAR_SLOTS: usize = 9;

/// The vanilla maximum stack size used by the port's restock planner.
pub const MAX_STACK: u32 = 64;

/// One stack as the pure rules see it: an item id and a count (components are carried by the
/// game glue, which copies real stacks; the planner only needs identity and amount).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotStack {
    pub item: String,
    pub count: u32,
}

impl SlotStack {
    pub fn new(item: &str, count: u32) -> Self {
        Self {
            item: item.to_owned(),
            count,
        }
    }
}

/// One restock move: take `count` of `inv_slot` and put it in `vest_slot`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub inv_slot: u32,
    pub vest_slot: usize,
    pub count: u32,
}

/// Whether `id` is one of the five vest items.
pub fn is_vest(id: &str) -> bool {
    VEST_IDS.contains(&id)
}

/// The container slot count of a vest tier (`banks * 9`), or `None` for a non-vest item.
pub fn slots_for(id: &str) -> Option<u32> {
    VEST_IDS
        .iter()
        .position(|vest| *vest == id)
        .map(|tier| u32::from(BANKS[tier]) * HOTBAR_SLOTS as u32)
}

/// The half-open slot range of `bank` (1-based) inside a container of `banks` banks.
pub fn bank_range(bank: u8, banks: u8) -> Option<(usize, usize)> {
    if bank == 0 || bank > banks {
        return None;
    }
    let start = (usize::from(bank) - 1) * HOTBAR_SLOTS;
    Some((start, start + HOTBAR_SLOTS))
}

/// Upstream `UVVestCapability.matchFilter`: vests never match; no filters matches everything;
/// otherwise a stack matches when any filter names the same item id.
pub fn matches_filter(filters: &[Option<String>], item: &str) -> bool {
    if is_vest(item) {
        return false;
    }
    if filters.iter().all(Option::is_none) {
        return true;
    }
    filters.iter().any(|filter| filter.as_deref() == Some(item))
}

/// Plan `collectItems`: move every inventory stack that matches the filters into the vest,
/// filling slots that already hold the same item before empty slots, capped at `MAX_STACK`.
/// Returns the moves in the order they should be applied; a caller that applies them to the
/// real container gets exactly what upstream's `insertWithOverflow` would store.
pub fn restock_plan(
    inventory: &[(u32, SlotStack)],
    filters: &[Option<String>],
    storage: &[Option<SlotStack>],
) -> Vec<Move> {
    let mut virtual_storage: Vec<Option<SlotStack>> = storage.to_vec();
    let mut moves = Vec::new();
    for (inv_slot, stack) in inventory {
        if is_vest(&stack.item) || !matches_filter(filters, &stack.item) || stack.count == 0 {
            continue;
        }
        let mut remaining = stack.count;
        // Stack onto slots already holding the same item first (upstream's first pass).
        for (slot, held) in virtual_storage.iter_mut().enumerate() {
            if remaining == 0 {
                break;
            }
            if let Some(held) = held
                && held.item == stack.item
                && held.count < MAX_STACK
            {
                let take = remaining.min(MAX_STACK - held.count);
                held.count += take;
                remaining -= take;
                moves.push(Move {
                    inv_slot: *inv_slot,
                    vest_slot: slot,
                    count: take,
                });
            }
        }
        // Then fill empty slots (upstream's second pass).
        for (slot, held) in virtual_storage.iter_mut().enumerate() {
            if remaining == 0 {
                break;
            }
            if held.is_none() {
                let take = remaining.min(MAX_STACK);
                *held = Some(SlotStack::new(&stack.item, take));
                remaining -= take;
                moves.push(Move {
                    inv_slot: *inv_slot,
                    vest_slot: slot,
                    count: take,
                });
            }
        }
    }
    moves
}

/// Plan `saveHotbar`: the nine `(slot, stack)` writes that copy the hotbar into `bank`.
/// `None` for a bank above the vest's tier.
pub fn save_plan(
    hotbar: &[Option<SlotStack>],
    bank: u8,
    banks: u8,
) -> Option<Vec<(usize, Option<SlotStack>)>> {
    let (start, _) = bank_range(bank, banks)?;
    Some(
        (0..HOTBAR_SLOTS)
            .map(|i| (start + i, hotbar.get(i).cloned().flatten()))
            .collect(),
    )
}

/// Plan `loadHotbar`: the nine stacks to put in the hotbar and the previous hotbar stacks that
/// must be returned to the inventory (anything non-empty).
pub fn load_plan(
    bank: u8,
    banks: u8,
    vest_slots: &[Option<SlotStack>],
    current_hotbar: &[Option<SlotStack>],
) -> Option<(Vec<Option<SlotStack>>, Vec<SlotStack>)> {
    let (start, _) = bank_range(bank, banks)?;
    let loaded: Vec<Option<SlotStack>> = (0..HOTBAR_SLOTS)
        .map(|i| vest_slots.get(start + i).cloned().flatten())
        .collect();
    let overflow: Vec<SlotStack> = current_hotbar
        .iter()
        .take(HOTBAR_SLOTS)
        .filter_map(|stack| stack.clone())
        .collect();
    Some((loaded, overflow))
}

/// The container slot `swap` targets: an explicit slot in range, or the first empty slot when
/// `idx` is `None` (upstream's `-1`). `None` when nothing can be swapped.
pub fn swap_index(idx: Option<usize>, storage: &[Option<SlotStack>]) -> Option<usize> {
    match idx {
        Some(slot) if slot < storage.len() => Some(slot),
        Some(_) => None,
        None => storage.iter().position(Option::is_none),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, count: u32) -> Option<SlotStack> {
        Some(SlotStack::new(item, count))
    }

    #[test]
    fn each_tier_has_banks_times_nine_slots() {
        assert_eq!(slots_for(VEST_IDS[0]), Some(9));
        assert_eq!(slots_for(VEST_IDS[1]), Some(18));
        assert_eq!(slots_for(VEST_IDS[2]), Some(27));
        assert_eq!(slots_for(VEST_IDS[3]), Some(36));
        assert_eq!(slots_for(VEST_IDS[4]), Some(45));
        assert_eq!(slots_for("minecraft:stone"), None);
    }

    #[test]
    fn only_the_five_vest_ids_are_vests() {
        assert!(is_vest(VEST_IDS[4]));
        assert!(!is_vest("minecraft:stone"));
        assert!(!is_vest("utilityvest:utility_vest"));
    }

    #[test]
    fn bank_ranges_are_nine_slots_and_bounds_checked() {
        assert_eq!(bank_range(1, 1), Some((0, 9)));
        assert_eq!(bank_range(1, 5), Some((0, 9)));
        assert_eq!(bank_range(5, 5), Some((36, 45)));
        assert_eq!(bank_range(0, 5), None);
        assert_eq!(bank_range(6, 5), None);
    }

    #[test]
    fn empty_filters_match_everything_but_a_vest_is_never_a_filter() {
        assert!(matches_filter(&[], "minecraft:stone"));
        assert!(matches_filter(&[None, None], "minecraft:stone"));
        assert!(matches_filter(
            &[Some("minecraft:stone".into())],
            "minecraft:stone"
        ));
        assert!(!matches_filter(
            &[Some("minecraft:stone".into())],
            "minecraft:dirt"
        ));
        assert!(!matches_filter(&[Some(VEST_IDS[0].into())], VEST_IDS[0]));
    }

    #[test]
    fn restock_fills_same_item_slots_before_empty_slots() {
        let inv = [(3, SlotStack::new("minecraft:stone", 80))];
        let filters = [Some("minecraft:stone".into())];
        let storage = [stack("minecraft:stone", 10), None, None];
        assert_eq!(
            restock_plan(&inv, &filters, &storage),
            vec![
                Move {
                    inv_slot: 3,
                    vest_slot: 0,
                    count: 54,
                },
                Move {
                    inv_slot: 3,
                    vest_slot: 1,
                    count: 26,
                },
            ]
        );
    }

    #[test]
    fn restock_leaves_unmatched_stacks_and_vests_alone() {
        let inv = [
            (0, SlotStack::new("minecraft:dirt", 5)),
            (1, SlotStack::new(VEST_IDS[0], 1)),
        ];
        let filters = [Some("minecraft:stone".into())];
        let storage = [None, None];
        assert!(restock_plan(&inv, &filters, &storage).is_empty());
    }

    #[test]
    fn save_writes_the_hotbar_into_the_selected_bank() {
        let hotbar = [
            stack("minecraft:stone", 3),
            None,
            stack("minecraft:dirt", 2),
        ];
        let got = save_plan(&hotbar, 2, 3).unwrap();
        assert_eq!(got.len(), 9);
        assert_eq!(got[0], (9, stack("minecraft:stone", 3)));
        assert_eq!(got[1], (10, None));
        assert_eq!(got[2], (11, stack("minecraft:dirt", 2)));
        assert_eq!(save_plan(&hotbar, 4, 3), None);
        assert_eq!(save_plan(&hotbar, 0, 3), None);
    }

    #[test]
    fn load_reads_the_bank_and_returns_the_old_hotbar_as_overflow() {
        let mut vest = vec![None; 27];
        vest[18] = stack("minecraft:stone", 5);
        let current = [stack("minecraft:dirt", 2), None];
        let (new_hotbar, overflow) = load_plan(3, 3, &vest, &current).unwrap();
        assert_eq!(new_hotbar[0], stack("minecraft:stone", 5));
        assert_eq!(new_hotbar[1], None);
        assert_eq!(overflow, vec![SlotStack::new("minecraft:dirt", 2)]);
        assert_eq!(load_plan(4, 3, &vest, &current), None);
    }

    #[test]
    fn swap_uses_an_explicit_slot_or_the_first_empty_one() {
        let storage = [
            stack("minecraft:stone", 1),
            None,
            stack("minecraft:dirt", 2),
        ];
        assert_eq!(swap_index(Some(2), &storage), Some(2));
        assert_eq!(swap_index(None, &storage), Some(1));
        assert_eq!(swap_index(Some(3), &storage), None);
        assert_eq!(swap_index(None, &[stack("minecraft:stone", 1)]), None);
    }
}
