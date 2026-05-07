# shellcheck shell=bash
# ============================================================================
# grantklassy.sh
# ============================================================================

# Bail on non-interactive shells
[ -z "$PS1" ] && return

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
# shellcheck disable=SC2154,SC1090
[ -f ~/.git-completion.bash ] && . "$_"

# ----------------------------------------------------------------------------
# macOS detection + setup (no-op on Linux)
# ----------------------------------------------------------------------------

if [[ "$(uname -s)" == "Darwin" ]]; then
  export RUNNING_ON_MAC=true
else
  export RUNNING_ON_MAC=false
fi

is_mac() {
  [[ "$RUNNING_ON_MAC" == true ]]
}

if is_mac; then
  bind '"\e[200~": paste-from-clipboard'
  bind '"\e[201~": end-paste-from-clipboard'

  MOUSEACCEL=$(defaults read .GlobalPreferences com.apple.mouse.scaling)
  if [[ "$MOUSEACCEL" != -1 ]]; then
    defaults write .GlobalPreferences com.apple.mouse.scaling -1
  fi

  eval "$(/opt/homebrew/bin/brew shellenv)"
  export BASH_SILENCE_DEPRECATION_WARNING=1
fi

# ----------------------------------------------------------------------------
# Export
# ----------------------------------------------------------------------------
export EDITOR="$(which vim)"

# ----------------------------------------------------------------------------
# Aliases
# ----------------------------------------------------------------------------

# Color
alias ls='ls -b --color'
alias la='ls -lah --color'
alias grep='grep --color=yes'

# Misc
alias src='cd ~/src'
alias h='history'
alias m='mount | column -t | less -S'
alias k='kubectl'
alias reload='source ~/.bashrc'
alias perms='stat -c "%a %A %G:%U %n" ./* | column -t'
alias claude='claude --model=opus --effort=max'

# ----------------------------------------------------------------------------
# Functions
# ----------------------------------------------------------------------------

# Usage: lines 2  -> prints two blank lines
#        lines 5  -> prints five blank lines
lines() {
  local n="${1:-1}"
  [[ "$n" =~ ^[0-9]+$ ]] || { printf 'usage: lines <nonnegative integer>\n' >&2; return 2; }
  for ((i = 0; i < n; i++)); do
    printf '\n'
  done
}

# Create .tar.gz of a directory
targz() {
  local cmd="tar cvzf ${1%%/}.tar.gz ${1%%/}/"
  echo "$cmd"
  eval "$cmd"
}

# Print/convert epoch timestamps
# No args: print current epoch
# With args: convert each from epoch to human-readable
epoch() {
  if [[ $# -eq 0 ]]; then
    date +%s
  else
    for arg in "$@"; do
      date -d "@${arg}"
    done
  fi
}

# ----------------------------------------------------------------------------
# Prompt
# ----------------------------------------------------------------------------

parse_branch() {
  if git rev-parse --git-dir > /dev/null 2>&1; then
    local branch
    branch=$(git symbolic-ref --short -q HEAD)
    printf " {git: %s}" "$branch"
  fi
}

# Colored prompt with git branch
export PS1="★★★ \[\033[01;31m\]\u@\h\[\033[00m\] ★★★ \[\033[01;34m\]\w\[\033[00m\]\$(parse_branch) ★★★ \$ "

