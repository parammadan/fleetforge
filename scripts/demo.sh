#!/usr/bin/env bash
#
# One command, one process, one URL.
#
# A leadership demonstration should not need two terminals, and it should not
# be shown through a development server — that is not what would ever run, and
# it is a second thing that can fail in front of an audience. This builds the
# production interface if it is stale, then serves it from the Rust binary
# alongside the API on a single loopback port.
#
# Ctrl-C stops everything. There is nothing left behind to stop separately.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# `cargo` lives in ~/.cargo/bin, which a non-login shell (CI, a GUI terminal,
# a script invoked by another script) does not always have on PATH.
command -v cargo >/dev/null 2>&1 || PATH="$HOME/.cargo/bin:$PATH"
export PATH

HOST="${FLEETFORGE_HOST:-127.0.0.1}"
PORT="${FLEETFORGE_PORT:-8080}"
EVIDENCE="${FLEETFORGE_EVIDENCE:-evidence/eks-recovery}"
UI_DIR="${FLEETFORGE_UI:-web/dist}"
PROFILE="${FLEETFORGE_PROFILE:-release}"
READY_TIMEOUT="${FLEETFORGE_READY_TIMEOUT:-60}"

die()  { printf '\nerror: %s\n' "$*" >&2; exit 1; }
step() { printf '  %s\n' "$*"; }

# --- Loopback only -----------------------------------------------------------
#
# FleetForge has no authentication of its own (THREAT_MODEL.md). Binding it to
# anything reachable would publish captured cluster state to the network, so
# a non-loopback host has to be asked for explicitly and out loud.
case "$HOST" in
  127.0.0.1|localhost|::1|"") ;;
  *)
    if [[ "${FLEETFORGE_ALLOW_NON_LOOPBACK:-}" != "yes" ]]; then
      die "refusing to bind $HOST: FleetForge has no authentication and must not be
       reachable from the network. Set FLEETFORGE_ALLOW_NON_LOOPBACK=yes if you
       genuinely mean it."
    fi
    printf '\nwarning: binding %s — this is reachable from the network and FleetForge\n' "$HOST" >&2
    printf '         has no authentication. Everything it serves is readable.\n\n' >&2
    ;;
esac

printf '\nFleetForge — EKS incident replay\n\n'

# --- Evidence ----------------------------------------------------------------
[[ -d "$EVIDENCE" ]] \
  || die "no evidence bundle at $EVIDENCE
       This demonstration replays a captured EKS incident. Without the bundle
       there is nothing to show, and nothing will be invented to fill the gap."
[[ -f "$EVIDENCE/35-fleetforge-events-complete.jsonl" ]] \
  || die "$EVIDENCE has no event log; it is not a replay bundle."

# --- Port --------------------------------------------------------------------
#
# Checked before building, so an occupied port costs a second rather than a
# two-minute compile.
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  holder="$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -Fcn 2>/dev/null \
            | awk '/^c/{c=substr($0,2)} /^n/{print c" on "substr($0,2)}' | head -1)"
  die "port $PORT is already in use${holder:+ by $holder}.
       Stop it, or choose another:  FLEETFORGE_PORT=8081 make demo"
fi

# --- Interface ---------------------------------------------------------------
#
# Rebuilt only when a source file is newer than the build, so a repeat demo
# starts immediately.
needs_build=no
if [[ ! -f "$UI_DIR/index.html" ]]; then
  needs_build=yes
elif [[ -n "$(find web/src web/index.html web/vite.config.ts web/package.json \
               -newer "$UI_DIR/index.html" 2>/dev/null | head -1)" ]]; then
  needs_build=yes
fi

if [[ "$needs_build" == yes ]]; then
  step "building the interface…"
  [[ -d web/node_modules ]] || (cd web && npm ci --silent)
  (cd web && npm run build --silent) >/dev/null \
    || die "the interface build failed; run 'cd web && npm run build' to see why."
else
  step "interface build is current"
fi

# --- Backend -----------------------------------------------------------------
BIN="target/$PROFILE/fleetforge"
build_flags=(-p ff-api)
[[ "$PROFILE" == release ]] && build_flags+=(--release)

step "building the backend ($PROFILE)…"
cargo build "${build_flags[@]}" 2>&1 | grep -E '^(error|warning: unused)' && die "backend build failed"
[[ -x "$BIN" ]] || die "no binary at $BIN after building."

# --- Run ---------------------------------------------------------------------
#
# The server runs in its own process group so the trap can take down anything
# it spawned, not just the parent. Without that, a Ctrl-C during startup can
# leave a listener holding the port — which is exactly the failure this script
# exists to prevent.
SERVER_PID=""
# Ctrl-C is how a demonstration ends, not how it fails. Without this, `make`
# reports "Error 130" to a presenter who did exactly the right thing.
STOPPED_DELIBERATELY=no
on_signal() { STOPPED_DELIBERATELY=yes; cleanup; }

cleanup() {
  local code=$?
  trap - EXIT INT TERM
  if [[ "$STOPPED_DELIBERATELY" == yes ]]; then code=0; fi
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    printf '\n  stopping…\n'
    kill -TERM -- "-$SERVER_PID" 2>/dev/null || kill -TERM "$SERVER_PID" 2>/dev/null || true
    for _ in $(seq 1 50); do
      kill -0 "$SERVER_PID" 2>/dev/null || break
      sleep 0.1
    done
    # Only if it ignored SIGTERM for five seconds.
    if kill -0 "$SERVER_PID" 2>/dev/null; then
      kill -KILL -- "-$SERVER_PID" 2>/dev/null || true
    fi
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  printf '  stopped.\n'
  exit "$code"
}
trap cleanup EXIT
trap on_signal INT TERM

step "starting the replay backend…"
set -m
"$BIN" --replay "$EVIDENCE" --ui "$UI_DIR" --bind "$HOST:$PORT" &
SERVER_PID=$!
set +m

# --- Readiness ---------------------------------------------------------------
#
# Polled against /readyz, then confirmed against the mode. A process that is
# listening is not the same as a process serving the right thing.
ready=no
for _ in $(seq 1 $((READY_TIMEOUT * 10))); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    die "the backend exited during startup. Its output is above."
  fi
  if curl -sf -o /dev/null "http://$HOST:$PORT/readyz" 2>/dev/null; then
    ready=yes
    break
  fi
  sleep 0.1
done
[[ "$ready" == yes ]] || die "the backend did not become ready within ${READY_TIMEOUT}s."

mode="$(curl -s "http://$HOST:$PORT/api/v1/environment" 2>/dev/null \
        | sed -n 's/.*"mode_label":"\([^"]*\)".*/\1/p')"
[[ "$mode" == "REPLAY" ]] \
  || die "the backend came up in mode '${mode:-unknown}', not REPLAY.
       Refusing to present fixture or live data as a captured incident."

curl -sf -o /dev/null "http://$HOST:$PORT/" \
  || die "the interface is not being served at /."

cat <<BANNER

  REPLAY — captured from a real EKS/Bottlerocket/Brupop run on 2026-09-13.
  The cluster no longer exists. Nothing here is live.

  →  http://$HOST:$PORT

  Ctrl-C to stop.

BANNER

# `wait` returns 128+signal when a trap fires. The trap has already decided
# what the exit status means.
wait "$SERVER_PID" || true
