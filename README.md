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
Its image is built in
[lemonfiber-caddy](https://github.com/lemonfiber/lemonfiber-caddy) from `caddy`,
with what the posture it runs under does not hold taken out. Every commit on
that repository's main publishes it and moves the pin here on a rolling pull
request; the release train tags this repository, and the bundle pins the tag.

This repository also holds the adapter that speaks lemonfiber's `proxy.front`
contract in front of Caddy: a Rust crate built on the core's adapter kit, into
the image `ghcr.io/lemonfiber/caddy-adapter`, published from every commit on
`main` and from each tag the release train cuts. It writes the routes and
certificates the core hands it as `caddy.json` into Caddy's configuration
directory, which it shares and Caddy only reads, and the core restarts Caddy to
take them up. Caddy's admin API is off.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `config/caddy/` | The files Caddy starts with, written into its configuration directory |
| `src/` | The adapter |
| `conformance/` | One recording per case of `proxy.front@1`, answered by the adapter in front of the pinned Caddy |
| `recordings/` | The recordings of the manifest's proofs |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against, and the core the adapter is built on |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
proof and every case of the adapter's contract against its recording, and every
pinned digest. The adapter is held to its
format, its lints, its tests, full line coverage and its dependency policy, and
its image runs as lemonfiber runs it. This repository holds no Python of its own.
`just ci` runs the gates that need nothing but this tree.
The image is built and run under the posture this manifest's main declares by
spec's
[`first-party-image`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-image.yml)
workflow in lemonfiber-caddy, on each of its pull requests and from its main.

## Licence

The code and data in this repository are under the licence in `LICENSE`.
`caddy`, which its image is built from, is under Apache-2.0, and the
image lemonfiber-caddy builds carries it under the same licence.
