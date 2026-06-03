# shellcheck shell=bash
# parse_branch prints " {git: BRANCH}" in a repo, nothing outside one.
if ! command -v git >/dev/null 2>&1; then
  printf '  (skipping parse_branch: git not on PATH)\n'
  gk_done
fi

# Positive: a fresh repo with one commit.
repo=$(cd "$(mktemp -d)" && pwd -P)
git init -q "$repo" >/dev/null 2>&1
git -C "$repo" -c user.email=t@example.com -c user.name=test \
    commit -q --allow-empty -m init >/dev/null 2>&1
assert_contains "$(cd "$repo" && parse_branch)" "{git:" "shows a branch inside a repo"

# Negative: a non-repo dir. GIT_CEILING_DIRECTORIES stops git walking up into a
# parent repo — critical because the dev box's $HOME is itself a git repo, which
# would otherwise make this dir look like it's "in a repo".
empty=$(cd "$(mktemp -d)" && pwd -P)
out=$(cd "$empty" && export GIT_CEILING_DIRECTORIES="$(dirname "$empty")" && parse_branch)
assert_eq "" "$out" "prints nothing outside a repo"

gk_done
