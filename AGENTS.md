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
- **One service, speaking no contract**, while the bundle assembles it. It
  declares `replaces` naming its own id, which is how it takes the id, the port
  and the hostname the bundle reserves for it (`ARCH-R222`).
- **Its claims' recordings sit under `recordings/caddy/`**, and its starting
  files, where it has any, under `config/caddy/` (`ARCH-R271`).
- **Every service declares `reaches` and `asks_for`** (`ARCH-R270`).
- **No Python here.** The reader is spec's `first-party-plugin` workflow's.
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **The adapter reaches Caddy through its admin socket alone**, at
  `/caddy/admin.sock` in the configuration directory it shares, and every
  configuration it loads keeps that socket owner-only. A configuration Caddy
  answers with anything other than what was loaded is refused, not served.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit `targets.toml` names, the same for every `lemonfiber-*` crate.
- **Moving Caddy's digest** means moving `RELEASE` and `DIGEST` in
  `src/adapter.rs` with it.
- **The image is built in `lemonfiber-caddy`** (`OPS-R93`). Its `Dockerfile` builds
  it from its upstream's, and it runs under the posture this manifest's main
  declares (`ARCH-R250`), with the starting files under `config/caddy/`. Its pin
  moves here on the release app's rolling pull request from there, so a change to
  the posture or the starting files is proved there on that repository's next run.
