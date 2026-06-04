# ============================================================================
# prompt.zsh — zsh prompt building blocks
# ============================================================================
# Sourced by ~/.zshrc AFTER lib.sh (needs host_color). This file only DEFINES
# the prompt's colour numbers; the entrypoint sets PROMPT (and PROMPT_SUBST).
# Kept separate from lib.sh because zsh builds prompt colours with %F{n}…%f,
# not the bare-ESC table — a different mechanism from bash's \[ \] wrapping.

pc_user=205               # pink   — username
pc_at=15                  # white  — the @ in user@host
pc_host=$(host_color)     # per-host hash colour — hostname
pc_path=208               # orange — working dir
