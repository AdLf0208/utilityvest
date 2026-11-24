#!/usr/bin/env bash
# In-game check for Utility Vest (SDK package, 0.1.0).
#
# The End is used so ambient item entities cannot pollute the item counts. The run
# proves: the five vest ids give and an unknown id is refused; all five carried
# recipes craft in a real table with their outputs consumed; the body equipment
# slot is written by /utilityvest wear and cleared by remove; the vest container
# stores and returns a parsed stack count; save/load restores a hotbar bank; and
# restock moves only item ids named by the saved bank.
#
#   GAME=<launcher-built game> CTL=<control> RES=<resources> tools/e2e.sh
#   ... SELFTEST=1     # the restock assertion is inverted: the run MUST fail
#   ... CONTROL=1      # hold a plain chestplate: every /utilityvest action refuses
#
# Every assertion reads a parsed ctl reply; the control and the inverted leg are
# what make the default run fail-capable. The cleanup trap always prints EXIT:.
set -Eeuo pipefail

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$CLONE/ports/utilityvest
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/utilityvest-e2e.sock}
LOG=${LOG:-$CLONE/ports/_shared/evidence/utilityvest/e2e-default.log}
SELFTEST=${SELFTEST:-0}
CONTROL=${CONTROL:-0}
WORLD=${WORLD:-UtilityVestE2E}

[ -n "$GAME" ] || { echo "GAME must name a launcher-built horizon-client carrying utilityvest.hmod" >&2; exit 1; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
command -v jq >/dev/null || { echo "missing jq" >&2; exit 1; }

mkdir -p "$(dirname "$LOG")" "$(dirname "$SOCK")"
exec > >(tee -a "$LOG") 2>&1

SCRATCH=$(mktemp -d /tmp/opencode/utilityvest-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data XDG_CONFIG_HOME=$SCRATCH/config HORIZON_NO_AUDIO=1
mkdir -p "$SCRATCH/config"
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { echo "+ $*"; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 30 >&2 || true; exit 1; }
cnt() { ctl inventory | jq -r --arg i "$1" '.counts[$i] // 0'; }
chat() { ctl messages 40 | jq -r '.chat[]'; }
cmd() { ctl chat "/$1" >/dev/null; ctl wait 3; }

finish() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 2
    rm -rf "$SCRATCH" 2>/dev/null || true
    printf 'EXIT:%s\n' "$status" >> "$LOG"
    echo "EXIT:$status"
}
trap finish EXIT

say "launch (offscreen renderer, creative, The End)"
rm -f "$SOCK"
ctl launch --game "$GAME" --world "$WORLD" --seed 20261124 --creative --offscreen --wait --resources "$RES"
ctl wait-ready 120
ctl command "time set noon"
ctl command "execute in minecraft:the_end run tp @s 0 100 0"
ctl wait 30
ctl command "fill -8 79 -8 8 79 12 minecraft:stone"
ctl command "kill @e[type=item]"
ctl wait 20

say "== the five vest ids and an unknown id =="
for tier in leather iron gold diamond netherite; do
    id="utilityvest:${tier}_utility_vest"
    ctl command "clear @s"
    ctl give "$id" 1 >/dev/null
    ctl wait 2
    [ "$(cnt "$id")" -ge 1 ] || fail "$id did not give"
done
ctl command "clear @s"
ctl give utilityvest:not_a_vest 1 >/dev/null 2>&1 || true
ctl wait 2
[ "$(cnt utilityvest:not_a_vest)" -eq 0 ] || fail "an unknown vest id was given"

say "== all five carried recipes craft in a real table =="
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" 6 80 2 || fail "the recipe check failed"

ctl command "clear @s"
ctl command "kill @e[type=item]"
ctl wait 5

if [ "$CONTROL" = 1 ]; then
    say "== control: a plain chestplate has no vest container =="
    ctl give minecraft:stick 1 >/dev/null
    ctl wait 2
    ctl select minecraft:stick >/dev/null
    ctl give minecraft:stone 16 >/dev/null
    ctl wait 2
    before=$(cnt minecraft:stone)
    cmd "utilityvest restock"
    after=$(cnt minecraft:stone)
    [ "$after" -eq "$before" ] || fail "control: restock moved $before -> $after without a vest"
    chat | grep -q "hold a vest" || fail "control: no refusal for a non-vest item"
    echo "PASS: control left $after stone and refused the action"
    exit 0
fi

say "== wear / remove prove the body equipment slot =="
ctl give utilityvest:leather_utility_vest 1 >/dev/null
ctl wait 2
ctl select utilityvest:leather_utility_vest >/dev/null
cmd "utilityvest wear"
body=$(ctl inventory | jq -r '.equipment["41"].item // .slots[]? | select(.slot == 41) | .item // empty' 2>/dev/null || true)
chat | grep -q "body slot 41" || fail "wear did not name the body slot"
cmd "utilityvest remove"
chat | grep -q "removed vest to slot" || fail "remove did not report the destination"
echo "  body slot probe: ${body:-<reported in chat>}"

say "== the vest container stores and returns a parsed count ==="
ctl select 1 >/dev/null
ctl give minecraft:stone 32 >/dev/null
ctl wait 2
ctl select 1 >/dev/null
[ "$(cnt minecraft:stone)" -ge 32 ] || fail "no stone to store"
cmd "utilityvest store 1"
after_store=$(cnt minecraft:stone)
chat | grep -q "stored 32 from slot 1" || fail "store feedback: $(chat | tail -3)"
[ "$after_store" -eq 0 ] || fail "store left $after_store stone in the inventory"
cmd "utilityvest take 0"
after_take=$(cnt minecraft:stone)
chat | grep -q "took 32 from vest slot 0" || fail "take feedback: $(chat | tail -3)"
[ "$after_take" -eq 32 ] || fail "take returned $after_take, not 32"

say "== save / load a hotbar bank =="
ctl select 1 >/dev/null
ctl give minecraft:dirt 7 >/dev/null
ctl wait 2
ctl select 1 >/dev/null
cmd "utilityvest save 1"
chat | grep -q "saved .* hotbar stack" || fail "save feedback: $(chat | tail -3)"
ctl command "clear @s minecraft:dirt"
ctl wait 2
[ "$(cnt minecraft:dirt)" -eq 0 ] || fail "clear did not remove the dirt"
cmd "utilityvest load 1"
chat | grep -q "loaded bank 1" || fail "load feedback: $(chat | tail -3)"
[ "$(cnt minecraft:dirt)" -eq 7 ] || fail "load restored $(cnt minecraft:dirt) dirt, not 7"

say "== restock moves only item ids named by the saved bank =="
# A fresh vest so the only saved filter is stone, then a dirt control.
ctl command "clear @s"
ctl give utilityvest:leather_utility_vest 1 >/dev/null
ctl wait 2
ctl select 1 >/dev/null
ctl give minecraft:stone 8 >/dev/null
ctl wait 2
ctl select 1 >/dev/null
cmd "utilityvest save 1"
ctl give minecraft:stone 16 >/dev/null
ctl wait 2
ctl give minecraft:dirt 5 >/dev/null
ctl wait 2
ctl select 1 >/dev/null
stone_before=$(cnt minecraft:stone)
dirt_before=$(cnt minecraft:dirt)
cmd "utilityvest restock"
stone_after=$(cnt minecraft:stone)
dirt_after=$(cnt minecraft:dirt)
echo "  stone $stone_before -> $stone_after, dirt $dirt_before -> $dirt_after"
if [ "$SELFTEST" = 1 ]; then
    # Inverted: the run must fail because a working restock moved the stone.
    [ "$stone_after" -eq "$stone_before" ] || fail "SELFTEST: restock moved $stone_before -> $stone_after, expected no move"
else
    chat | grep -q "restocked .* item" || fail "restock feedback: $(chat | tail -3)"
    [ "$stone_after" -lt "$stone_before" ] || fail "restock did not move any stone"
    [ "$dirt_after" -eq "$dirt_before" ] || fail "restock moved dirt, which is not a bank filter"
fi

echo "PASS: five vest ids give and an unknown id is refused, all five recipes craft with their outputs consumed, wear/remove write and clear the body equipment slot, store/take move a parsed 32, save/load restore seven dirt, and restock moves only the bank-named stone while leaving dirt (control refuses without a vest; SELFTEST inverts the move)"
exit 0
