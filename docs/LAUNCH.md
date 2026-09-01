# Launch drafts (not posted)

Maintainer: hypery11.

## Short

Oh My Muse Code (omm) is out: the missing productivity layer for Meta Muse Code.

Native plugin: 19 role skills, 19 slash-commands, 8 hooks.
Ralph Stop actually blocks (decision: block on Muse 1.0.1).
omm setup / omm doctor work. team/ask/hud CLI still stubs.

https://github.com/hypery11/oh-my-musecode

Experimental: MUSE_EXPERIMENTAL_PLUGINS=1

## Show HN

Show HN: Oh My Muse Code — an oh-my-* plugin for Meta Muse Code

Muse Code 1.0.1 shipped experimental native plugins. There was not yet an oh-my-claudecode-style harness on that API.

oh-my-musecode is Muse-native (not a fork, not an API proxy):

- 19 role skills (architect through git-master)
- 19 slash-commands (/ralph, /team, /verify, ...)
- 8 hooks, including a Ralph Stop loop that emits {"decision":"block"} while .omm/ralph.json is active
- companion CLI omm (setup / doctor)

Install:

export MUSE_EXPERIMENTAL_PLUGINS=1
muse plugins marketplace add omm https://github.com/hypery11/oh-my-musecode
muse plugins install oh-my-musecode@omm
muse plugins approve oh-my-musecode

Honest limits: not 1:1 with oh-my-claudecode. No tmux HUD. No Meta proxy.

MIT. https://github.com/hypery11/oh-my-musecode
