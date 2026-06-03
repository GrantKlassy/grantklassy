#!/bin/sh
# ============================================================================
# run.sh — test install.rs across Linux distros in Docker
# ============================================================================
# For each distro: build a cached image (prereqs + nightly), then run
# in-container.sh against a fresh snapshot of the repo's WORKING-TREE state
# (committed + new whitelisted files, with uncommitted edits) so the code under
# test is exactly what's on disk now — not just what's committed.
#
#   sh run.sh                 # default matrix: fedora ubuntu debian
#   sh run.sh fedora          # a subset
#
# Needs a running Docker daemon — on macOS: `colima start`.
set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(cd -- "$here/../../.." && pwd)   # the $HOME checkout (repo root)
distros=${*:-fedora ubuntu debian}

[ -d "$repo_root/.grantklassy" ] || { echo "repo root not found at $repo_root" >&2; exit 1; }
command -v docker >/dev/null 2>&1 || { echo "docker not on PATH" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "docker daemon unreachable — run 'colima start'" >&2; exit 1; }

# Snapshot the working tree's tracked + new-whitelisted files (NOT ignored ones).
# --cached = committed/staged; --others --exclude-standard = new files git would
# track (e.g. the not-yet-committed shell/ tree), honoring .gitignore.
# Keep it under $HOME: Colima bind-mounts the home dir into the VM, but the macOS
# TMPDIR (/var/folders/...) is NOT shared, so a mktemp there can't be -v mounted.
snapdir=$(mktemp -d "$repo_root/.gk-snap.XXXXXX")
snap="$snapdir/gk.tar"
( cd "$repo_root" && git ls-files -z --cached --others --exclude-standard | tar --null -T - -cf "$snap" ) \
  || { echo "snapshot failed" >&2; exit 1; }
echo "repo snapshot: $(tar -tf "$snap" | wc -l | tr -d ' ') files -> $snap"

fails=0
summary=""
for d in $distros; do
  img="gk-test-$d"
  printf '\n############### %s — build ###############\n' "$d"
  if ! docker build -f "$here/Dockerfile.$d" -t "$img" "$here"; then
    summary="$summary
  $d : FAIL (build)"
    fails=$(( fails + 1 ))
    continue
  fi
  printf '\n############### %s — test ###############\n' "$d"
  if docker run --rm \
        -v "$snap:/gk.tar:ro" \
        -v "$here/in-container.sh:/in-container.sh:ro" \
        "$img" sh /in-container.sh; then
    summary="$summary
  $d : PASS"
  else
    summary="$summary
  $d : FAIL (assertions)"
    fails=$(( fails + 1 ))
  fi
done

rm -rf "$snapdir"
printf '\n================= MATRIX =================%s\n' "$summary"
if [ "$fails" -eq 0 ]; then
  echo "all distros green"
  exit 0
fi
echo "$fails distro(s) failed" >&2
exit 1
