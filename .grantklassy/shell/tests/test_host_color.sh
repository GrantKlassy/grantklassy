# shellcheck shell=bash
# FLAGSHIP cross-shell invariant. Running this file under BOTH bash and zsh must
# yield the SAME colour for a host, even though zsh arrays are 1-indexed and bash
# 0-indexed (host_color adds +1 only under zsh). The constants are precomputed,
# so a regression in either shell's indexing makes that shell — and only that
# shell — fail here. cksum("fixedhost")=2258191700, 2258191700 % 16 = 4,
# palette[4] = 190.
export HOST=fixedhost HOSTNAME=fixedhost
assert_eq 190 "$(host_color)" "host_color(fixedhost) = 190 in both shells"

# The domain is stripped to the short host, so the colour is unchanged.
export HOST=fixedhost.example.com HOSTNAME=fixedhost.example.com
assert_eq 190 "$(host_color)" "host_color strips the domain"

# A different host hashes to a different (specific) palette entry.
export HOST=mbp HOSTNAME=mbp
assert_eq 99 "$(host_color)" "host_color(mbp) = 99"

gk_done
