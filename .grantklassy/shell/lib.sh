# shellcheck shell=bash
# ============================================================================
# lib.sh — shared, side-effect-free shell library for grantklassy
# ============================================================================
# Sourced by BOTH ~/.bashrc.d/grantklassy.sh (bash) and ~/.zshrc (zsh), right
# after each entrypoint's interactive guard. Sourcing this file ONLY defines
# functions and variables — it performs no side effects: no setopt/shopt, no
# compinit, no PATH mutation, no `defaults`/`brew`, no prompt install. That is
# precisely what makes the helpers unit-testable: the harness in shell/tests/
# sources THIS file directly, in a non-interactive shell, and exercises each
# function without the entrypoint's `return` guard ever firing.
#
# Cross-shell discipline honored throughout: use only constructs valid in BOTH
# bash and zsh ($'...', [[ =~ ]], C-style `for`, printf, `local` inside
# functions), and branch on $ZSH_VERSION only where the shells genuinely differ
# (array indexing in host_color).
#
# shellcheck disable=SC2034  # the color vars are defined for interactive use

# ----------------------------------------------------------------------------
# Platform predicate (pure)
# ----------------------------------------------------------------------------
# is_mac honors $RUNNING_ON_MAC when an entrypoint has exported it (true/false)
# and otherwise falls back to `uname -s`. The fallback is what lets the test
# harness source lib.sh on any OS yet still force either branch by setting
# RUNNING_ON_MAC — and it means merely sourcing this file never shells out.
is_mac() {
  if [ -n "${RUNNING_ON_MAC:-}" ]; then
    [ "$RUNNING_ON_MAC" = true ]
  else
    [ "$(uname -s)" = Darwin ]
  fi
}

# ----------------------------------------------------------------------------
# PATH helper (pure until called)
# ----------------------------------------------------------------------------
# Prepend $1 to PATH iff it isn't already present. Defining it is a no-op;
# only calling it mutates PATH, so the entrypoints call it while sourcing here
# stays clean. The case-glob guard makes repeated calls (e.g. via `reload`)
# idempotent — PATH never grows on a re-source.
path_prepend() {
  case ":$PATH:" in
    *":$1:"*) ;;
    *) PATH="$1:$PATH" ;;
  esac
}

# ----------------------------------------------------------------------------
# Misc helpers
# ----------------------------------------------------------------------------

# Print N blank lines. `lines 3` -> three blank lines. Non-numeric -> rc 2.
lines() {
  local n="${1:-1}" i
  [[ "$n" =~ ^[0-9]+$ ]] || { printf 'usage: lines <nonnegative integer>\n' >&2; return 2; }
  for (( i = 0; i < n; i++ )); do printf '\n'; done
}

# Create dir.tar.gz from a directory (a trailing slash on the arg is tolerated).
# Quoted expansions, no eval — names with spaces work, and a missing/invalid
# arg returns 2 instead of the old behavior (empty $1 made the eval'd command
# `tar cvzf .tar.gz /`, i.e. an attempt to tar the root filesystem).
targz() {
  local dir="${1%/}"
  if [ -z "$dir" ] || [ ! -d "$dir" ]; then
    printf 'usage: targz <directory>\n' >&2
    return 2
  fi
  printf 'tar cvzf %s.tar.gz %s/\n' "$dir" "$dir"
  tar cvzf "$dir.tar.gz" "$dir/"
}

# epoch        -> current unix epoch
# epoch N [N…] -> convert each from epoch to human-readable
# `date -d @N` is GNU-only; macOS/BSD `date` needs `date -r N`. Routing through
# is_mac fixes the long-standing bash-on-macOS breakage where this helper only
# ever issued `date -d` and so failed on the Mac it ships to.
epoch() {
  local arg
  if [ "$#" -eq 0 ]; then
    date +%s
  elif is_mac; then
    for arg in "$@"; do date -r "$arg"; done
  else
    for arg in "$@"; do date -d "@$arg"; done
  fi
}

# gkdisk — reformat an external USB stick or SSD as exFAT, interactively (macOS).
# Thin wrapper so the util is runnable by name. The script lives under macos/;
# the previous wrappers pointed at ~/.grantklassy/gkdisk.rs (no macos/) and so
# never found it.
gkdisk() {
  "$HOME/.grantklassy/macos/gkdisk.rs" "$@"
}

# ----------------------------------------------------------------------------
# Colors (ANSI escape codes)
# ----------------------------------------------------------------------------
# Each var holds a real ESC byte (via $'...', valid in bash and zsh), so it
# works with plain printf/echo and inside other strings — no `echo -e` needed.
# Names match the classic bash-colors table: B=bold, U=underline, On_=background,
# I=high-intensity, BI=bold high-intensity, On_I=high-intensity background.
# NOTE: these BARE-ESC vars are for echo/cecho. A prompt needs differently
# wrapped escapes (bash \[ \]; zsh %F{}), which live in prompt.bash / prompt.zsh.

# Reset
Color_Off=$'\e[0m'

# Regular
Black=$'\e[0;30m'; Red=$'\e[0;31m'; Green=$'\e[0;32m'; Yellow=$'\e[0;33m'
Blue=$'\e[0;34m'; Purple=$'\e[0;35m'; Cyan=$'\e[0;36m'; White=$'\e[0;37m'

# Bold
BBlack=$'\e[1;30m'; BRed=$'\e[1;31m'; BGreen=$'\e[1;32m'; BYellow=$'\e[1;33m'
BBlue=$'\e[1;34m'; BPurple=$'\e[1;35m'; BCyan=$'\e[1;36m'; BWhite=$'\e[1;37m'

# Underline
UBlack=$'\e[4;30m'; URed=$'\e[4;31m'; UGreen=$'\e[4;32m'; UYellow=$'\e[4;33m'
UBlue=$'\e[4;34m'; UPurple=$'\e[4;35m'; UCyan=$'\e[4;36m'; UWhite=$'\e[4;37m'

# Background
On_Black=$'\e[40m'; On_Red=$'\e[41m'; On_Green=$'\e[42m'; On_Yellow=$'\e[43m'
On_Blue=$'\e[44m'; On_Purple=$'\e[45m'; On_Cyan=$'\e[46m'; On_White=$'\e[47m'

# High intensity
IBlack=$'\e[0;90m'; IRed=$'\e[0;91m'; IGreen=$'\e[0;92m'; IYellow=$'\e[0;93m'
IBlue=$'\e[0;94m'; IPurple=$'\e[0;95m'; ICyan=$'\e[0;96m'; IWhite=$'\e[0;97m'

# Bold high intensity
BIBlack=$'\e[1;90m'; BIRed=$'\e[1;91m'; BIGreen=$'\e[1;92m'; BIYellow=$'\e[1;93m'
BIBlue=$'\e[1;94m'; BIPurple=$'\e[1;95m'; BICyan=$'\e[1;96m'; BIWhite=$'\e[1;97m'

# High intensity backgrounds
On_IBlack=$'\e[0;100m'; On_IRed=$'\e[0;101m'; On_IGreen=$'\e[0;102m'; On_IYellow=$'\e[0;103m'
On_IBlue=$'\e[0;104m'; On_IPurple=$'\e[0;105m'; On_ICyan=$'\e[0;106m'; On_IWhite=$'\e[0;107m'

# Styles
Bold=$'\e[1m'; Dim=$'\e[2m'; Italic=$'\e[3m'; Underline=$'\e[4m'
Blink=$'\e[5m'; Reverse=$'\e[7m'; Hidden=$'\e[8m'; Strike=$'\e[9m'

# cecho <color> <text...> — print text in a color, then reset.
# e.g. cecho "$BRed" "build failed". printf-based so it behaves identically in
# bash and zsh (zsh's `print` is unnecessary and would diverge).
cecho() {
  local color=$1; shift
  printf '%s%s%s\n' "$color" "$*" "$Color_Off"
}

# ----------------------------------------------------------------------------
# git prompt helper
# ----------------------------------------------------------------------------
# Print " {git: BRANCH}" when inside a work tree on a named branch; print
# nothing when not in a repo OR on a detached HEAD (the symbolic-ref guard).
# Used by both prompts via command substitution, so it re-runs every prompt.
parse_branch() {
  git rev-parse --git-dir >/dev/null 2>&1 || return
  local branch
  branch=$(git symbolic-ref --short -q HEAD 2>/dev/null) || return
  printf ' {git: %s}' "$branch"
}

# ----------------------------------------------------------------------------
# Per-host prompt color
# ----------------------------------------------------------------------------
# Deterministic per-host color: hash the short hostname into a palette index so
# a given machine always gets the same recognizable prompt color. Curated
# 256-color codes, all readable on a dark background; 205 (user) and 208 (path)
# are intentionally absent so the host stands apart.
host_color_palette=(196 202 214 220 190 154 118 82 48 51 45 39 75 99 171 213)

# zsh arrays are 1-indexed, bash 0-indexed, so the SAME logical element needs
# index (h % n) in bash but (h % n) + 1 in zsh. Branching on $ZSH_VERSION keeps
# both shells selecting the identical color for a host — the invariant the test
# harness asserts by running one test file under both shells. The hostname is
# read from $HOST (set by zsh) or $HOSTNAME (set by bash), falling back to
# `hostname`, so the two shells never hash a different name.
host_color() {
  local host hash n idx
  host=${HOST:-${HOSTNAME:-$(hostname)}}
  host=${host%%.*}                                   # short host (up to first dot)
  hash=$(printf '%s' "$host" | cksum | cut -d' ' -f1)
  n=${#host_color_palette[@]}
  if [ -n "${ZSH_VERSION:-}" ]; then
    idx=$(( hash % n + 1 ))
  else
    idx=$(( hash % n ))
  fi
  printf '%s' "${host_color_palette[idx]}"
}
