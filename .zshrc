# ============================================================================
# .zshrc — interactive zsh entrypoint
# ============================================================================
# Thin wrapper. The testable helpers (colours, cecho, lines, targz, epoch,
# parse_branch, host_color, is_mac, path_prepend, gkdisk) live in the shared,
# side-effect-free ~/.grantklassy/shell/lib.sh, exercised by shell/tests/. This
# file is intentionally just the zsh-specific SIDE EFFECTS: the interactive
# guard, history/PATH/completion setup, macOS tweaks, aliases, and the prompt.
# Its bash twin is ~/.bashrc.d/grantklassy.sh; the two stay mirror-able.

# Bail on non-interactive shells
[[ -o interactive ]] || return

# ----------------------------------------------------------------------------
# Load the shared library + the zsh prompt pieces
# ----------------------------------------------------------------------------
# ${(%):-%x} is the path of this file; :A:h -> its absolute dir (=$HOME for a
# normal .zshrc). Fall back to the canonical $HOME checkout layout.
_gk_shell="${${(%):-%x}:A:h}/.grantklassy/shell"
[[ -d "$_gk_shell" ]] || _gk_shell="$HOME/.grantklassy/shell"
source "$_gk_shell/lib.sh"
source "$_gk_shell/prompt.zsh"
unset _gk_shell

# Platform flag — consumed by is_mac (lib.sh) and the macOS block below.
if [[ "$(uname -s)" == Darwin ]]; then export RUNNING_ON_MAC=true; else export RUNNING_ON_MAC=false; fi

# ----------------------------------------------------------------------------
# Shell (history, PATH, completions)
# ----------------------------------------------------------------------------

# INC_APPEND_HISTORY writes each command immediately so concurrent windows don't
# clobber each other's history; HIST_IGNORE_DUPS keeps the up-arrow list usable.
setopt APPEND_HISTORY INC_APPEND_HISTORY SHARE_HISTORY HIST_IGNORE_DUPS
HISTSIZE=1000
SAVEHIST=1000
HISTFILE=${HISTFILE:-$HOME/.zsh_history}

# Tied unique-array: zsh drops duplicate PATH entries automatically, so
# re-sourcing via `reload` doesn't grow PATH each time.
typeset -U path PATH
path+=(/usr/local/sbin /usr/sbin)

# Common user bin dirs, newest-wins (prepend). path_prepend (lib.sh) is
# idempotent, so re-running from `reload` is safe.
for _dir in "$HOME/bin" "$HOME/.cargo/bin" "$HOME/.local/bin"; do
  [[ -d "$_dir" ]] && path_prepend "$_dir"
done
unset _dir
export PATH

# zsh completion system. Cheap once; cached on disk after that.
autoload -Uz compinit && compinit -u

# ----------------------------------------------------------------------------
# macOS setup (no-op off macOS)
# ----------------------------------------------------------------------------
# Wrapped in a function: top-level `local` is silently a no-op in zsh (and a
# hard error in bash), so the previous bare `local` lines here were a latent
# cross-shell bug. Each write is guarded on the current value (idempotent).
_gk_macos_setup() {
  local cur
  cur=$(defaults read .GlobalPreferences com.apple.mouse.scaling 2>/dev/null)
  [[ "$cur" == -1 ]] || defaults write .GlobalPreferences com.apple.mouse.scaling -1
  # -float 10.0/1.0 read back as 10/1.
  cur=$(defaults read -g InitialKeyRepeat 2>/dev/null)
  [[ "$cur" == 10 ]] || defaults write -g InitialKeyRepeat -float 10.0
  cur=$(defaults read -g KeyRepeat 2>/dev/null)
  [[ "$cur" == 1 ]] || defaults write -g KeyRepeat -float 1.0

  # brew shellenv: PATH/MANPATH/HOMEBREW_PREFIX. Apple Silicon, Intel fallback.
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  fi
}
is_mac && _gk_macos_setup

# ----------------------------------------------------------------------------
# Export
# ----------------------------------------------------------------------------
export EDITOR="$(command -v vim || echo vi)"

# ----------------------------------------------------------------------------
# Aliases
# ----------------------------------------------------------------------------

# ls colour flag: GNU coreutils uses --color, BSD ls (macOS) uses -G. Pick at
# load time.
if ls --color=auto / >/dev/null 2>&1; then
  alias ls='ls --color=auto'
  alias la='ls -lah --color=auto'
else
  alias ls='ls -G'
  alias la='ls -lahG'
fi
alias grep='grep --color=auto'

# Misc — same set as the bash rc
alias src='cd ~/src'
alias h='history'
alias m='mount | column -t | less -S'
alias k='kubectl'
alias reload='[[ -f ~/.zshenv ]] && source ~/.zshenv; source ~/.zshrc'
alias perms='stat -c "%a %A %G:%U %n" ./* | column -t'

# ----------------------------------------------------------------------------
# Prompt
# ----------------------------------------------------------------------------
# pc_* come from prompt.zsh; parse_branch from lib.sh. PROMPT_SUBST re-expands
# $(parse_branch) and the %F{$pc_*} colours on every prompt.
# %n=user %m=short host %~=cwd; %F{n}…%f colour spans.
setopt PROMPT_SUBST
PROMPT='★★★ %F{$pc_user}%n%F{$pc_at}@%F{$pc_host}%m%f ★★★ %F{$pc_path}%~%f$(parse_branch) ★★★ %# '

# CF CLI completions
[[ -f "/Users/grantklassy/.config/cf/completions/_cf.zsh" ]] && source "/Users/grantklassy/.config/cf/completions/_cf.zsh"
