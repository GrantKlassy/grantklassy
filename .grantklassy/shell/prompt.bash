# shellcheck shell=bash
# ============================================================================
# prompt.bash — bash prompt building blocks
# ============================================================================
# Sourced by ~/.bashrc.d/grantklassy.sh AFTER lib.sh (needs host_color). This
# file only DEFINES the prompt's colour pieces; the entrypoint assembles and
# exports PS1. Kept separate from lib.sh because prompt escapes are
# shell-specific: bash needs every SGR wrapped in \[ \] so readline counts the
# prompt width correctly, which is why the bare-ESC $Red-style table (for echo)
# can't be reused here.

# _prompt_color <code> -> a \[ \]-wrapped bold 256-colour SGR. PC_RESET clears it.
_prompt_color() { printf '%s' "\[\033[1;38;5;$1m\]"; }
PC_RESET='\[\033[00m\]'
PC_USER=$(_prompt_color 205)               # pink   — username
PC_AT=$(_prompt_color 15)                  # white  — the @ in user@host
PC_HOST=$(_prompt_color "$(host_color)")   # per-host hash colour — hostname
PC_PATH=$(_prompt_color 208)               # orange — working dir
