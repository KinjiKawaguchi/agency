#!/bin/bash
# Tests scripts/check-bindings.sh in a throwaway repository, with a stand-in for cargo that writes the bindings: they
# pass when they are as committed, fail when a type changed, was removed or is new, and pass when the directory
# cannot be removed and a .nfs file is left in it (the way a shared NFS disk behaves, which broke the check).
#
#   scripts/test/check-bindings.sh
#
# CHECK_SCRIPT names the script to test instead of the real one, so that a mutated copy can be tried.
set -euo pipefail
cd "$(dirname "$0")/../.."
check_script="${CHECK_SCRIPT:-$PWD/scripts/check-bindings.sh}"

work="$(mktemp -d)"
cleanup() { chmod -R u+w "$work" 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

# The stand-in for `cargo test ... export_bindings`: writes A.ts and B.ts as committed, unless FAKE_A says another A,
# FAKE_NO_B leaves B out (a type removed in Rust), or FAKE_NEW adds a type.
mkdir -p "$work/bin"
cat > "$work/bin/cargo" <<'FAKE'
#!/bin/bash
mkdir -p bindings
printf '%s\n' "${FAKE_A:-export type A = string}" > bindings/A.ts
[ -n "${FAKE_NO_B:-}" ] || printf 'export type B = number\n' > bindings/B.ts
[ -z "${FAKE_NEW:-}" ] || printf '%s\n' "$FAKE_NEW" > bindings/New.ts
FAKE
chmod +x "$work/bin/cargo"

# fresh_repo: a repository whose bindings/ holds A.ts and B.ts, as committed.
fresh_repo() {
  chmod -R u+w "$work/repo" 2>/dev/null || true
  rm -rf "$work/repo"
  mkdir -p "$work/repo/scripts" "$work/repo/bindings"
  cp "$check_script" "$work/repo/scripts/check-bindings.sh"
  cd "$work/repo"
  git init -q -b main
  git config user.email t@example.test
  git config user.name t
  git config commit.gpgsign false
  printf 'export type A = string\n' > bindings/A.ts
  printf 'export type B = number\n' > bindings/B.ts
  git add -A && git commit -qm "chore: base"
  cd - >/dev/null
}
# run <env assignments...>: runs the script in the repository; sets status and output.
run() {
  status=0
  output="$(cd "$work/repo" && env PATH="$work/bin:$PATH" "$@" scripts/check-bindings.sh 2>&1)" || status=$?
}

fresh_repo; run
[ "$status" -eq 0 ] || fail "bindings as committed fail the check: $output"

fresh_repo; run FAKE_A='export type A = number'
[ "$status" -eq 1 ] || fail "a changed type: wanted 1, got $status"
case "$output" in *"bindings/A.ts"*) ;; *) fail "a changed type: the output does not name bindings/A.ts: $output" ;; esac

fresh_repo; run FAKE_NO_B=1
[ "$status" -eq 1 ] || fail "a removed type: wanted 1, got $status"
case "$output" in *"bindings/B.ts"*) ;; *) fail "a removed type: the output does not name bindings/B.ts: $output" ;; esac

fresh_repo; run FAKE_NEW='export type New = boolean'
[ "$status" -eq 1 ] || fail "a new type: wanted 1, got $status"
case "$output" in *"bindings/New.ts"*) ;; *) fail "a new type: the output does not name bindings/New.ts: $output" ;; esac

# A leftover .nfs file is no new type.
fresh_repo; : > "$work/repo/bindings/.nfs00000000000a0001"; run
[ "$status" -eq 0 ] || fail "a .nfs file in bindings/ fails the check: $output"
[ -e "$work/repo/bindings/.nfs00000000000a0001" ] || fail "the .nfs file was removed: it is not the script's to remove"

# The directory cannot be removed (the repository's root is read-only, as an NFS directory with a file still open is
# not removable): the check still passes, since it deletes the files and not the directory.
if [ "$(id -u)" -eq 0 ]; then
  echo "skipped: running as root, so a read-only directory does not stop the removal"
else
  fresh_repo; chmod 555 "$work/repo"; run
  chmod 755 "$work/repo"
  [ "$status" -eq 0 ] || fail "bindings/ that cannot be removed fail the check: $output"
fi

[ "$failures" -eq 0 ] && echo "check-bindings: ok"
exit "$failures"
