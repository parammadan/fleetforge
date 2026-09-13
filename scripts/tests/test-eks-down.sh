#!/usr/bin/env bash
# Regression test for scripts/eks-down.sh.
#
# Guards one observed failure: on 2026-09-13 the script exited 0 having
# destroyed nothing. The piped confirmation answered the script's own prompt,
# Terraform asked for its own approval, received EOF, and aborted — and the
# script reported success while the cluster kept running and billing.
#
# A teardown script that can silently no-op is worse than no script, because it
# converts "the cluster is gone" from a fact into a belief. These cases assert
# it cannot happen again.
#
# Runs entirely against a stub `terraform` on PATH. Touches no cloud, no
# kubectl, no real state.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/eks-down.sh"
PASS=0
FAIL=0

# Builds an isolated TF_DIR plus a stub `terraform` with the given behaviour.
# $1 destroy exit code
# $2 state list BEFORE destroy (newline separated, may be empty)
# $3 state list AFTER  destroy
make_stub() {
  local rc=$1 before=$2 after=$3
  local dir; dir=$(mktemp -d)
  mkdir -p "$dir/tfdir" "$dir/bin"
  # Marker file so the stub knows whether destroy has run yet.
  # Trailing newline, like real terraform. The absence of one previously hid a
  # counting bug in the script, so the stub must not be tidier than reality.
  if [ -n "$before" ]; then printf '%s\n' "$before" > "$dir/before.txt"; else : > "$dir/before.txt"; fi
  if [ -n "$after"  ]; then printf '%s\n' "$after"  > "$dir/after.txt";  else : > "$dir/after.txt";  fi
  cat > "$dir/bin/terraform" <<STUB
#!/usr/bin/env bash
case "\$1" in
  destroy)
    touch "$dir/destroyed"
    exit $rc
    ;;
  state)
    if [ -f "$dir/destroyed" ]; then cat "$dir/after.txt"; else cat "$dir/before.txt"; fi
    exit 0
    ;;
  *) exit 0 ;;
esac
STUB
  chmod +x "$dir/bin/terraform"
  # eks-down.sh short-circuits when there is no state and no .terraform dir.
  touch "$dir/tfdir/terraform.tfstate"
  mkdir -p "$dir/tfdir/.terraform"
  printf '%s' "$dir"
}

run_case() {
  local name=$1 rc=$2 before=$3 after=$4 expect=$5
  local dir; dir=$(make_stub "$rc" "$before" "$after")
  local out
  out=$(printf 'destroy fleetforge-demo\n' \
    | env PATH="$dir/bin:$PATH" FLEETFORGE_TF_DIR="$dir/tfdir" bash "$SCRIPT" 2>&1)
  local got=$?
  if { [ "$expect" = "zero" ] && [ "$got" -eq 0 ]; } || { [ "$expect" = "nonzero" ] && [ "$got" -ne 0 ]; }; then
    printf '  PASS  %-52s exit=%s\n' "$name" "$got"; PASS=$((PASS+1))
  else
    printf '  FAIL  %-52s exit=%s (wanted %s)\n' "$name" "$got" "$expect"; FAIL=$((FAIL+1))
    printf '%s\n' "$out" | tail -6 | sed 's/^/          /'
  fi
  rm -rf "$dir"
}

echo "eks-down.sh regression tests"

# The original bug: terraform aborts on EOF, returns non-zero, nothing destroyed.
run_case "terraform fails (EOF on prompt) -> must not exit 0" \
  1 "module.eks.aws_eks_cluster.this" "module.eks.aws_eks_cluster.this" nonzero

# Terraform lies: exits 0 but state is unchanged.
run_case "exit 0 but zero resources destroyed -> must not exit 0" \
  0 "module.eks.aws_eks_cluster.this
module.vpc.aws_vpc.this" "module.eks.aws_eks_cluster.this
module.vpc.aws_vpc.this" nonzero

# Partial destroy: some resources survive.
run_case "partial destroy, resources remain -> must not exit 0" \
  0 "module.eks.aws_eks_cluster.this
module.vpc.aws_vpc.this" "module.vpc.aws_vpc.this" nonzero

# The success path must still succeed, or the guards are useless.
run_case "clean destroy, state emptied -> must exit 0" \
  0 "module.eks.aws_eks_cluster.this
module.vpc.aws_vpc.this" "" zero

# Nothing to destroy is not a failure.
run_case "empty state to begin with -> must exit 0" \
  0 "" "" zero

echo
echo "  ${PASS} passed, ${FAIL} failed"
[ "$FAIL" -eq 0 ]
