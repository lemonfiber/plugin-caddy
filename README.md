# Caddy for lemonfiber

**Friendly hostnames instead of ports.**
Caddy is one of the parts the stack lemonfiber ships is built from.
Without it: Use localhost:8989 and friends.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
lemonfiber assembles Caddy into the stack it ships as a stack service.
Its image is built here from `caddy`, with what the posture it runs under
does not hold taken out. Every commit on main publishes it, the release train
tags this repository, and the bundle pins the tag.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `config/caddy/` | The files Caddy starts with, written into its configuration directory |
| `Dockerfile` | How its image is built |
| `targets.toml` | The lemonfiber its checks run against |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
claim against its recording, and every pinned digest. This repository holds no
Python of its own. `just ci` runs the gates that need nothing but this tree.
The image is one job of spec's
[`first-party-image`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-image.yml)
workflow: built and run under the posture its manifest declares on a pull
request, and published, run again and pinned from main and a train tag.

## Licence

The data in this repository is under the licence in `LICENSE`.
`caddy`, which its image is built from, is under Apache-2.0, and the
image built here carries it under the same licence.
