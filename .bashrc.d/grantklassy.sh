# shellcheck shell=bash
# ============================================================================
# grantklassy.sh — interactive bash entrypoint
# ============================================================================
# Thin wrapper. The testable helpers (colours, cecho, lines, targz, epoch,
# parse_branch, host_color, is_mac, path_prepend, gkdisk) live in the shared,
# side-effect-free ~/.grantklassy/shell/lib.sh, exercised by shell/tests/. This
# file is intentionally just the bash-specific SIDE EFFECTS: the interactive
# guard, history/PATH/completion setup, macOS tweaks, aliases, and the prompt.
# Its zsh twin is ~/.zshrc; the two are kept mirror-able section-for-section.

# Bail on non-interactive shells
[ -z "$PS1" ] && return

# ----------------------------------------------------------------------------
# Load the shared library + the bash prompt pieces
# ----------------------------------------------------------------------------
# Resolve shell/ from THIS file's path ($0 is unreliable when sourced in bash;
# BASH_SOURCE is correct), falling back to the canonical $HOME checkout layout.
_gk_shell="${BASH_SOURCE[0]%/*}/../.grantklassy/shell"
[ -d "$_gk_shell" ] || _gk_shell="$HOME/.grantklassy/shell"
# shellcheck source=/dev/null
. "$_gk_shell/lib.sh"
# shellcheck source=/dev/null
. "$_gk_shell/prompt.bash"
unset _gk_shell

# Platform flag — consumed by is_mac (lib.sh) and the macOS block below.
if [ "$(uname -s)" = Darwin ]; then export RUNNING_ON_MAC=true; else export RUNNING_ON_MAC=false; fi

# ----------------------------------------------------------------------------
# Shell (history, PATH, completions)
# ----------------------------------------------------------------------------
shopt -s histappend
HISTFILESIZE=1000
HISTSIZE=1000

PATH=$PATH:/usr/local/sbin:/usr/sbin

# shellcheck source=/dev/null
[ -f /etc/bash_completion ] && source /etc/bash_completion
# shellcheck source=/dev/null
[ -f ~/.bash_functions ] && source ~/.bash_functions
# shellcheck source=/dev/null
[ -f ~/.git-completion.bash ] && . ~/.git-completion.bash

# ----------------------------------------------------------------------------
# macOS setup (no-op off macOS)
# ----------------------------------------------------------------------------
# Wrapped in a function so its temporaries are `local` (top-level `local` is a
# hard error in bash) and so the side effects only run when actually on a Mac.
_gk_macos_setup() {
  # Bracketed paste so pasted text isn't executed line-by-line.
  bind '"\e[200~": paste-from-clipboard' 2>/dev/null
  bind '"\e[201~": end-paste-from-clipboard' 2>/dev/null

  # Mouse accel off; fast key repeat; natural scroll off. Each write is guarded
  # on the current value so it's idempotent and we don't needlessly poke prefs.
  local cur
  cur=$(defaults read .GlobalPreferences com.apple.mouse.scaling 2>/dev/null)
  [ "$cur" = -1 ] || defaults write .GlobalPreferences com.apple.mouse.scaling -1
  cur=$(defaults read NSGlobalDomain KeyRepeat 2>/dev/null)
  [ "$cur" = 1 ] || defaults write NSGlobalDomain KeyRepeat -int 1
  cur=$(defaults read NSGlobalDomain InitialKeyRepeat 2>/dev/null)
  [ "$cur" = 10 ] || defaults write NSGlobalDomain InitialKeyRepeat -int 10
  cur=$(defaults read NSGlobalDomain com.apple.swipescrolldirection 2>/dev/null)
  [ "$cur" = 0 ] || defaults write NSGlobalDomain com.apple.swipescrolldirection -bool false

  # brew shellenv: PATH/MANPATH/HOMEBREW_PREFIX. Apple Silicon, Intel fallback.
  if [ -x /opt/homebrew/bin/brew ]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [ -x /usr/local/bin/brew ]; then
    eval "$(/usr/local/bin/brew shellenv)"
  fi
  export BASH_SILENCE_DEPRECATION_WARNING=1
}
is_mac && _gk_macos_setup

# ----------------------------------------------------------------------------
# Export
# ----------------------------------------------------------------------------
export EDITOR="$(command -v vim || echo vi)"

# ----------------------------------------------------------------------------
# Aliases
# ----------------------------------------------------------------------------

# ls/grep colour — flag differs: GNU coreutils uses --color, BSD ls (macOS
# default) uses -G. Detect at load time so the alias works on either. (The old
# bash alias hard-coded --color and errored on macOS's BSD ls.)
if ls --color=auto / >/dev/null 2>&1; then
  alias ls='ls --color=auto'
  alias la='ls -lah --color=auto'
else
  alias ls='ls -G'
  alias la='ls -lahG'
fi
alias grep='grep --color=auto'

# Misc — same set as the zsh rc
alias src='cd ~/src'
alias h='history'
alias m='mount | column -t | less -S'
alias k='kubectl'
alias reload='source ~/.bashrc'
alias perms='stat -c "%a %A %G:%U %n" ./* | column -t'
alias claude='claude --model=opus --effort=max'

# ----------------------------------------------------------------------------
# Prompt
# ----------------------------------------------------------------------------
# PC_* and parse_branch come from prompt.bash + lib.sh (sourced above). Escapes
# are wrapped in \[ \] there so readline counts the width right; parse_branch
# is left unexpanded (\$(...)) so it re-runs each prompt.
export PS1="★★★ ${PC_USER}\u${PC_AT}@${PC_HOST}\h${PC_RESET} ★★★ ${PC_PATH}\w${PC_RESET}\$(parse_branch) ★★★ \$ "
