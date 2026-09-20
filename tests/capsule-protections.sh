#!/usr/bin/env bash
#
# Does the capsule a user receives actually enforce what it promises?
#
# Everything else in this project tests components: the crypto round-trips, the
# orchestrator talks to a mock, the package refuses a tampered chunk. None of
# that exercises the artifact a recipient is handed. This does — it builds real
# capsules, runs them, and checks what they do.
#
# Each case states what it is trying to break. A test that only proves a capsule
# opens when everything is correct proves almost nothing: the interesting
# question is whether it refuses when something is wrong, and whether a refusal
# leaves anything behind.
#
# Usage: tests/capsule-protections.sh <path-to-nyedarch-buildtool>
set -uo pipefail

B="${1:-./target/release/nyedarch-buildtool}"
WORK="${WORK:-/tmp/nyedarch-protections}"
MARKER="SECRET-PAYLOAD-MARKER-$$"
PASS="Protections-passphrase-2026"

pass_n=0; fail_n=0
ok()   { echo "  PASS  $1"; pass_n=$((pass_n+1)); }
bad()  { echo "  FAIL  $1"; echo "::error::capsule protection: $1"; fail_n=$((fail_n+1)); }

rm -rf "$WORK"; mkdir -p "$WORK/src/nested"
echo "$MARKER" > "$WORK/src/secret.txt"
echo "nested $MARKER" > "$WORK/src/nested/deep.txt"

# Seal and stage a capsule, echoing its path.
seal() {
  local proj="$1"; shift
  rm -rf "$proj"
  echo accept | "$B" seal "$WORK/src" "$proj" "$PASS" "$@" > "$proj.log" 2>&1
  ( cd "$proj" && sh stage-capsule.sh > /dev/null 2>&1 )
  ls "$proj"/*.nyarch 2>/dev/null | head -1
}

# Run a capsule and report how many files it produced.
run() {
  local cap="$1" out="$2" pass="${3:-$PASS}"
  rm -rf "$out"
  printf '%s\n' "$pass" | timeout 90 "$cap" "$out" > "$out.log" 2>&1
  find "$out" -type f 2>/dev/null | wc -l | tr -d ' '
}

# No denied run may leave the payload anywhere under the output path.
no_plaintext() {
  ! grep -rqs "$MARKER" "$1" 2>/dev/null
}

echo "== baseline: a correct capsule must open =="
CAP=$(seal "$WORK/p-base")
if [ -z "$CAP" ]; then
  bad "sealing produced no capsule; nothing else can be tested"
  exit 1
fi
[ "$(run "$CAP" "$WORK/o-base")" -ge 2 ] \
  && ok "correct passphrase on the creator machine extracts the tree" \
  || bad "a correct capsule refused its own creator"

echo "== passphrase =="
n=$(run "$CAP" "$WORK/o-wrong" "not-the-passphrase")
{ [ "$n" = "0" ] && no_plaintext "$WORK/o-wrong"; } \
  && ok "wrong passphrase refused, nothing written" \
  || bad "wrong passphrase produced $n file(s) or left plaintext"

echo "== machine authorization =="
# A different machine identity must not open it. Namespaces give a genuinely
# different hostname and machine-id without touching this host.
if command -v unshare > /dev/null 2>&1; then
  echo "deadbeefdeadbeefdeadbeefdeadbeef" > "$WORK/other-machine-id"
  rm -rf "$WORK/o-other"
  unshare --mount --uts bash -c "
      hostname nyedarch-other-host
      mount --bind '$WORK/other-machine-id' /etc/machine-id 2>/dev/null
      printf '%s\n' '$PASS' | timeout 90 '$CAP' '$WORK/o-other' > '$WORK/o-other.log' 2>&1
  " > /dev/null 2>&1
  n=$(find "$WORK/o-other" -type f 2>/dev/null | wc -l | tr -d ' ')
  { [ "$n" = "0" ] && no_plaintext "$WORK/o-other"; } \
    && ok "a different machine is refused, nothing written" \
    || bad "a different machine extracted $n file(s)"
else
  echo "  SKIP  no unshare available to simulate another machine"
fi

echo "== time window =="
# Built for a window three hours from now: must refuse now.
FUTURE=$(date -u -d '+3 hours' +%H:%M 2>/dev/null || date -u -v+3H +%H:%M)
CAP_T=$(seal "$WORK/p-time" --time "$FUTURE" --time-tolerance-min 5 --tz-offset-min 0)
n=$(run "$CAP_T" "$WORK/o-time")
{ [ "$n" = "0" ] && no_plaintext "$WORK/o-time"; } \
  && ok "outside the permitted window: refused" \
  || bad "a capsule opened $n file(s) outside its time window"

# And inside the window it must open, or the check above proves nothing.
NOW=$(date -u +%H:%M)
CAP_TN=$(seal "$WORK/p-time-now" --time "$NOW" --time-tolerance-min 45 --tz-offset-min 0)
[ "$(run "$CAP_TN" "$WORK/o-time-now")" -ge 2 ] \
  && ok "inside the permitted window: opens" \
  || bad "a capsule refused inside its own time window"

echo "== tamper =="
# The embedded package is authenticated: any edit to it must be refused.
PKG="$WORK/p-base/capsule.nyeda"
OFF=$(python3 -c "
cap=open('$CAP','rb').read(); pkg=open('$PKG','rb').read()
print(cap.find(pkg))")
if [ "$OFF" -ge 0 ] 2>/dev/null; then
  tampered=0
  for delta in 8 64 200; do
    cp "$CAP" "$WORK/tampered.nyarch"
    printf '\x41' | dd of="$WORK/tampered.nyarch" bs=1 seek=$((OFF+delta)) count=1 conv=notrunc 2>/dev/null
    chmod +x "$WORK/tampered.nyarch"
    n=$(run "$WORK/tampered.nyarch" "$WORK/o-tamper-$delta")
    if [ "$n" != "0" ] || ! no_plaintext "$WORK/o-tamper-$delta"; then
      tampered=$((tampered+1))
    fi
  done
  [ "$tampered" = "0" ] \
    && ok "every edit to the sealed package is refused" \
    || bad "$tampered edit(s) to the sealed package still extracted"
else
  bad "could not locate the sealed package inside the capsule"
fi

echo "== binding =="
# A payload must not be transplantable into another capsule. Two capsules built
# from the same source have different keys and bindings, so swapping the sealed
# package between them must fail.
CAP_B=$(seal "$WORK/p-other")
if [ -n "$CAP_B" ] && [ "$OFF" -ge 0 ] 2>/dev/null; then
  python3 - << PY
cap = bytearray(open("$CAP_B","rb").read())
other = open("$PKG","rb").read()
own = open("$WORK/p-other/capsule.nyeda","rb").read()
i = cap.find(own)
if i >= 0 and len(other) == len(own):
    cap[i:i+len(other)] = other
    open("$WORK/transplant.nyarch","wb").write(bytes(cap))
PY
  if [ -f "$WORK/transplant.nyarch" ]; then
    chmod +x "$WORK/transplant.nyarch"
    n=$(run "$WORK/transplant.nyarch" "$WORK/o-transplant")
    { [ "$n" = "0" ] && no_plaintext "$WORK/o-transplant"; } \
      && ok "a payload moved into another capsule is refused" \
      || bad "a transplanted payload extracted $n file(s)"
  else
    echo "  SKIP  packages differ in length; transplant not constructible here"
  fi
fi

echo "== one-shot =="
CAP_O=$(seal "$WORK/p-oneshot" --one-shot)
[ "$(run "$CAP_O" "$WORK/o-oneshot")" -ge 2 ] \
  && ok "one-shot capsule extracts once" \
  || bad "a one-shot capsule refused its only run"
sleep 3
[ -f "$CAP_O" ] \
  && bad "a one-shot capsule survived a successful extraction" \
  || ok "one-shot capsule removed after extraction"

echo
echo "protections: $pass_n passed, $fail_n failed"
exit $(( fail_n > 0 ? 1 : 0 ))
