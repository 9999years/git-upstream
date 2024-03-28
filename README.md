# git-upstream

[![Crates.io](https://img.shields.io/crates/v/git-upstream)](https://crates.io/crates/git-upstream)

A shortcut for `git push --set-upstream "$(git remote)" "$(git rev-parse --abbrev-ref HEAD)"`.

Usage: `git-upstream [--fail-fast] [--branch BRANCH] [--remote REMOTE]`.

Unless `--fail-fast` is given, all remotes are tried until one succeeds.


## Installation

With Nix, you can `nix run github:9999years/git-upstream -- ...`.

You can also `cargo install git-upstream`.

Finally, statically linked binaries are uploaded to GitHub for each release.
