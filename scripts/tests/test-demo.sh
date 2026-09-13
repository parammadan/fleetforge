#!/usr/bin/env bash
#
# Regression tests for scripts/demo.sh.
#
# The launcher's job is to fail loudly and leave nothing behind. Every case here
# is one where a plausible implementation succeeds quietly, presents the wrong
# data, or orphans a process holding the port — the failures that show up in
# front of an audience rather than in CI.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEMO="$ROOT/scripts/demo.sh"

# `cargo` lives in ~/.cargo/bin, which a non-login shell (CI, a GUI terminal,
# a script invoked by another script) does not always have on PATH.
command -v cargo >/dev/null 2>&1 || PATH="$HOME/.cargo/bin:$PATH"
export PATH

PASS=0; FAIL=0

# A port nothing else is likely to want.
PORT="${DEMO_TEST_PORT:-8137}"

result() { # name expected_ok actual_code [detail]
  local name="$1" want="$2" code="$3" detail="${4:-}"
  local ok=no
  if [[ "$want" == ok && "$code" -eq 0 ]] || [[ "$want" == fail && "$code" -ne 0 ]]; then ok=yes; fi
  if [[ "$ok" == yes ]]; then
    printf '  PASS  %-56s exit=%s\n' "$name" "$code"; PASS=$((PASS+1))
  else
    printf '  FAIL  %-56s exit=%s %s\n' "$name" "$code" "$detail"; FAIL=$((FAIL+1))
  fi
}

contains() { # haystack_file needle name
  if grep -qF "$2" "$1"; then
    printf '  PASS  %-56s\n' "$3"; PASS=$((PASS+1))
  else
    printf '  FAIL  %-56s (missing: %s)\n' "$3" "$2"; FAIL=$((FAIL+1))
    printf '        output was:\n'; sed 's/^/        | /' "$1" | head -12
  fi
}

port_free() { ! lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }

echo "demo.sh regression tests"

# Build once up front. Otherwise the first real start in case 5 compiles inside
# the readiness loop and the whole suite looks like a hang.
( cd "$ROOT" && cargo build -p ff-api >/dev/null 2>&1 ) || { echo "  FAIL  backend does not build"; exit 1; }
[[ -f "$ROOT/web/dist/index.html" ]] || ( cd "$ROOT/web" && npm run build --silent >/dev/null 2>&1 )

port_free "$PORT" || { echo "  SKIP  port $PORT is in use; set DEMO_TEST_PORT"; exit 0; }

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"; pkill -f "fleetforge --replay $ROOT/evidence" 2>/dev/null' EXIT

# --- 1. Missing evidence -----------------------------------------------------
#
# Must refuse rather than start and show an empty control room. An empty replay
# is indistinguishable from a cluster where nothing happened.
FLEETFORGE_EVIDENCE=/nonexistent/evidence FLEETFORGE_PORT="$PORT" \
  "$DEMO" >"$OUT/1.log" 2>&1
result "missing evidence bundle -> must not start" fail "$?"
contains "$OUT/1.log" "no evidence bundle" "  ...and says which bundle is missing"

# --- 2. Evidence directory without an event log ------------------------------
mkdir -p "$OUT/empty-bundle"
FLEETFORGE_EVIDENCE="$OUT/empty-bundle" FLEETFORGE_PORT="$PORT" \
  "$DEMO" >"$OUT/2.log" 2>&1
result "evidence dir with no event log -> must not start" fail "$?"
contains "$OUT/2.log" "not a replay bundle" "  ...and says it is not a replay bundle"

# --- 3. Occupied port --------------------------------------------------------
#
# Detected before the build, so it costs a second rather than a compile.
python3 -c "
import socket,time,sys
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
s.bind(('127.0.0.1',$PORT)); s.listen(1)
sys.stderr.write('held\n'); sys.stderr.flush()
time.sleep(90)
" 2>"$OUT/holder.err" &
HOLDER=$!
for _ in $(seq 1 50); do grep -q held "$OUT/holder.err" 2>/dev/null && break; sleep 0.1; done

FLEETFORGE_PORT="$PORT" "$DEMO" >"$OUT/3.log" 2>&1
result "occupied port -> must not start" fail "$?"
contains "$OUT/3.log" "already in use" "  ...and names the conflict"
contains "$OUT/3.log" "FLEETFORGE_PORT=8081" "  ...and offers the fix"
kill "$HOLDER" 2>/dev/null; wait "$HOLDER" 2>/dev/null

# --- 4. Non-loopback bind is refused by default ------------------------------
#
# FleetForge has no authentication. Binding it anywhere reachable publishes
# captured cluster state to the network.
FLEETFORGE_HOST=0.0.0.0 FLEETFORGE_PORT="$PORT" "$DEMO" >"$OUT/4.log" 2>&1
result "0.0.0.0 bind -> refused without explicit opt-in" fail "$?"
contains "$OUT/4.log" "has no authentication" "  ...and says why"

# --- 5. A real start: ready, REPLAY, UI served, then clean shutdown ----------
port_free "$PORT" || { echo "  FAIL  port not released after failure cases"; FAIL=$((FAIL+1)); }

# Started in its own session, and signalled by process group — which is what a
# terminal does on Ctrl-C.
#
# This matters: POSIX requires a shell to ignore SIGINT in a job it starts in
# the *background*, and an ignored signal cannot be trapped. Sending SIGINT to a
# backgrounded `demo.sh` therefore does nothing, and a test written that way
# reports a shutdown bug that does not exist. Ask the question the way the user
# asks it.
#
# The launcher is also given a *default* SIGINT disposition. A shell sets SIGINT
# to SIG_IGN in any job it backgrounds, that disposition is inherited across
# fork and exec, and a signal ignored on entry cannot be trapped — so a test run
# from a background shell would hand demo.sh a SIGINT it is unable to catch and
# then report a shutdown bug that does not exist. This is not the launcher
# accommodating the test; it is the test reproducing a terminal.
DEMO_PID="$(python3 -c "
import os, signal, subprocess, sys
def default_signals():
    for s in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP, signal.SIGQUIT):
        signal.signal(s, signal.SIG_DFL)
p = subprocess.Popen(sys.argv[1:], start_new_session=True,
                     preexec_fn=default_signals,
                     stdout=open('$OUT/5.log','w'), stderr=subprocess.STDOUT,
                     env={**os.environ,
                          'FLEETFORGE_PORT':'$PORT','FLEETFORGE_PROFILE':'debug'})
print(p.pid)
" "$DEMO")"
[[ -n "$DEMO_PID" ]] || { echo "  FAIL  could not start demo.sh"; exit 1; }
started=no
for _ in $(seq 1 1800); do
  curl -sf -o /dev/null "http://127.0.0.1:$PORT/readyz" 2>/dev/null && { started=yes; break; }
  kill -0 "$DEMO_PID" 2>/dev/null || break
  sleep 0.1
done

if [[ "$started" == yes ]]; then
  printf '  PASS  %-56s\n' "starts and becomes ready"; PASS=$((PASS+1))

  mode=$(curl -s "http://127.0.0.1:$PORT/api/v1/environment" | sed -n 's/.*"mode_label":"\([^"]*\)".*/\1/p')
  [[ "$mode" == REPLAY ]] \
    && { printf '  PASS  %-56s\n' "serves REPLAY, not fixture or live"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s got %s\n' "serves REPLAY" "$mode"; FAIL=$((FAIL+1)); }

  # The interface itself, from the production build, on the same port.
  code=$(curl -s -o "$OUT/index.html" -w '%{http_code}' "http://127.0.0.1:$PORT/")
  [[ "$code" == 200 ]] && grep -q '<div id="root">' "$OUT/index.html" \
    && { printf '  PASS  %-56s\n' "serves the production interface at /"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s http=%s\n' "serves the production interface at /" "$code"; FAIL=$((FAIL+1)); }

  # A hashed asset, which only exists in a production build.
  asset=$(grep -o '/assets/[^"]*\.js' "$OUT/index.html" | head -1)
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT$asset")
  [[ -n "$asset" && "$code" == 200 ]] \
    && { printf '  PASS  %-56s\n' "serves hashed production assets"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s asset=%s http=%s\n' "serves hashed production assets" "$asset" "$code"; FAIL=$((FAIL+1)); }

  # Loopback only: the same port must not answer on a routable address.
  ip=$(ipconfig getifaddr en0 2>/dev/null || true)
  if [[ -n "$ip" ]]; then
    if curl -sf -o /dev/null --max-time 2 "http://$ip:$PORT/readyz" 2>/dev/null; then
      printf '  FAIL  %-56s reachable on %s\n' "listens on loopback only" "$ip"; FAIL=$((FAIL+1))
    else
      printf '  PASS  %-56s\n' "listens on loopback only"; PASS=$((PASS+1))
    fi
  else
    printf '  SKIP  %-56s (no routable address)\n' "listens on loopback only"
  fi

  # An unknown API path must be JSON 404, not the single-page fallback.
  ct=$(curl -s -o /dev/null -w '%{http_code} %{content_type}' "http://127.0.0.1:$PORT/api/v1/nope")
  [[ "$ct" == "404 application/json" ]] \
    && { printf '  PASS  %-56s\n' "unknown API path is a JSON 404"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s got %s\n' "unknown API path is a JSON 404" "$ct"; FAIL=$((FAIL+1)); }

  contains "$OUT/5.log" "http://127.0.0.1:$PORT" "prints exactly one URL"

  # --- 6. Clean shutdown ----------------------------------------------------
  #
  # Signal the process group, as a terminal does. See the note above the start.
  kill -INT -- "-$DEMO_PID" 2>/dev/null || kill -INT "$DEMO_PID" 2>/dev/null
  gone=no
  for _ in $(seq 1 100); do
    kill -0 "$DEMO_PID" 2>/dev/null || { gone=yes; break; }
    sleep 0.1
  done
  [[ "$gone" == yes ]] \
    && { printf '  PASS  %-56s\n' "Ctrl-C stops the launcher"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s\n' "Ctrl-C stops the launcher"; FAIL=$((FAIL+1)); }

  # The one that actually matters: nothing left holding the port.
  released=no
  for _ in $(seq 1 100); do
    port_free "$PORT" && { released=yes; break; }
    sleep 0.1
  done
  [[ "$released" == yes ]] \
    && { printf '  PASS  %-56s\n' "no orphan process holds the port"; PASS=$((PASS+1)); } \
    || { printf '  FAIL  %-56s\n' "no orphan process holds the port"; FAIL=$((FAIL+1)); }
else
  printf '  FAIL  %-56s\n' "starts and becomes ready"; FAIL=$((FAIL+1))
  sed 's/^/        | /' "$OUT/5.log" | tail -20
  kill -INT -- "-$DEMO_PID" 2>/dev/null || kill -TERM "$DEMO_PID" 2>/dev/null
fi

echo
printf '  %d passed, %d failed\n\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]]
