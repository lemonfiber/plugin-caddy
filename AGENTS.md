# AGENTS.md — plugin-caddy

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

A bundled part: Caddy, as the stack lemonfiber ships carries it, and the adapter
that speaks the `proxy.front@1` contract in front of it. The adapter is the Rust
crate at the root, built on `lemonfiber-adapter-kit` from the core at the commit
`Cargo.toml` pins, into the image the `Dockerfile` builds. `release.yml`
publishes that image from every commit on `main` and from the train's tag. The
[bundle contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/bundle.md)
says how the catalogue pins this repository and how lemonfiber carries it.

## The rules you cannot break

- **The plugin id is the service's id** (`ARCH-R266`). The container, the
  configuration directory, the settings and the journal keep their names.
- **Two services: `caddy` and `caddy-adapter`.** `caddy` speaks no contract and
  declares `replaces` naming its own id, which is how it takes the id, the port
  and the hostname the bundle reserves for it (`ARCH-R222`). `caddy-adapter`
  fronts it, speaks `proxy.front@1`, provides `proxy.front` and shares Caddy's
  configuration directory at `/caddy` (`ARCH-R339`).
- **Its proofs' recordings sit under `recordings/caddy/`**, and its starting
  files under `config/caddy/` (`ARCH-R271`).
- **`conformance/` holds one recording per case of `proxy.front@1`**, written
  by `lemonfiber plugin record .` at the commit `targets.toml` names, the
  adapter in front of Caddy at the digest `plugin.toml` pins; CI takes every
  recording again with the adapter built from this tree. Re-record whenever the
  adapter's answers or Caddy's digest change. `tests/conformance.rs` reads
  every recording with the core's own client.
- **Every service declares `reaches` and `asks_for`** (`ARCH-R270`).
- **No Python here.** The reader is spec's `first-party-plugin` workflow's.
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **Caddy reads `caddy.json`, a JSON configuration the adapter writes**, with
  its admin API off and its configuration directory mounted read-only
  (`config_ro`), so nothing a route carries is read as Caddyfile text and
  nothing reaches Caddy while it runs. The adapter writes `caddy.json` and the
  table it came from, `routing.json`, through
  `lemonfiber_adapter_kit::shared`, and `apply` and `reload` answer on-restart
  (`ARCH-R341`), so the core restarts Caddy to take a table up. A
  configuration that is not the one rendered from the table is refused, not
  served.
- **Every value is held to its grammar before anything is written**: a name is
  a DNS name, never a wildcard and never the name of two routes; a host is a DNS
  name or an IPv4 address; a port is not zero; a certificate or key is a plain
  file beneath the configuration directory and never Caddy's own files.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit `targets.toml` names, the same for every `lemonfiber-*` crate.
- **Moving Caddy's digest** means moving `RELEASE` and `DIGEST` in
  `src/adapter.rs` with it; `tests/manifest.rs` fails until they name the
  image the manifest's `caddy` runs.
- **The image is built in `lemonfiber-caddy`** (`OPS-R93`). Its `Dockerfile` builds
  it from its upstream's, and it runs under the posture this manifest's main
  declares (`ARCH-R250`), with the starting files under `config/caddy/`. Its pin
  moves here on the release app's rolling pull request from there, so a change to
  the posture or the starting files is proved there on that repository's next run.
