# Vantage

> An independent, unofficial client compatible with Minecraft: Bedrock Edition. Not approved by
> or associated with Mojang or Microsoft. Minecraft is a trademark of Microsoft Corporation.

> [!NOTE]
> Vantage is a fork of [Cinnabar](https://github.com/cinnabar-mc/cinnabar) by cinnabar-mc, taken
> at `08446aa`. It keeps Cinnabar's client and adds the Vantage HUD and world tools running
> natively inside it. Cinnabar's work stays under its own license; see [FORK.md](FORK.md) and
> [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

> [!WARNING]
> Vantage is a non-vanilla client and still in development. Its movement and combat may not yet
> match the official client exactly, so server anticheats may flag you and some servers may ban
> you. Play on servers at your own risk.

Vantage by Crawl: HUD elements (Keystrokes, CPS, Coordinates, Armor status, Potion status and
more), world tools (chunk borders, hitboxes, WAILA, block outline, waypoints) and mechanics (zoom,
fullbright, freelook, CPS limiter) built into a Minecraft: Bedrock Edition client written in Rust.
The Vantage mod is also available on its own as a wasm component for the upstream host.

## Download

- **Windows installer and the Vantage wasm mod:** [latest release](https://github.com/crawl4u/Vantage/releases/latest)

On first launch, Vantage downloads the vanilla resource pack after you accept the Minecraft EULA.
No Mojang assets are bundled. The release notes explain how to open unsigned builds.
No sound? See [docs/audio-troubleshooting.md](docs/audio-troubleshooting.md).

## Build and run

```sh
make play
```

This fetches the vanilla assets, builds the Go core and opens the launcher. Sign in with the
Microsoft device code; the token stays in `.local/auth/`, so never share or commit it. Use
`make play PROFILE=release` for an optimised build, and `make help` for every target.

On Debian/Ubuntu, install `libwayland-dev` and `libudev-dev` first.

## What's different

Vanilla behaviour is the default. Everything below is extra:

| Feature | What it does |
| --- | --- |
| **Performance** | Frame pacing, latency and chunk streaming built to beat vanilla. |
| **1.12 PvP feel** | Ships with Bedrock 1.12-style combat visuals: the classic attack swing, hurt flash, enchantment glint and view bob, plus Java-style first-person hand sway and nametags. |
| **Live resource packs** | Add, remove or reorder packs without leaving the world. |
| **Multi-account manager** | Sign in with several Microsoft accounts and switch between them from the launcher, no signing out. |
| **Custom skins** | Import custom-geometry (4D/5D) skins and skin packs. See [docs/custom-skins.md](docs/custom-skins.md). |
| **Discord** | Rich presence, plus joining and inviting friends through Discord. See [docs/discord.md](docs/discord.md). |
| **Cinnabar players** | A badge on other Cinnabar players in the tab list, and update notices as soon as a release ships. See [docs/cinnabar-api.md](docs/cinnabar-api.md). |
| **Mods** *(preview)* | Personal WebAssembly mods with reload and explicit grants, plus paired server/client mods with declared blocks, items and typed messages. Start with [the mod guide](docs/modding/src/appendix/paired-mods.md); server client-code execution currently requires the developer switch. |
| **JSON-UI editor** | Preview and edit pack UI exactly as Cinnabar renders it, at [jsonui.cinnabar.gg](https://jsonui.cinnabar.gg/). |
| **Figma to JSON-UI** | A Figma plugin that turns a frame into a JSON-UI screen and resource pack. [Download it](https://jsonui.cinnabar.gg/figma-plugin.zip), or see [tools/figma-plugin](tools/figma-plugin/README.md). |

## How it works

```text
cinnabar (Rust, Bevy/wgpu)  ── local socket ──  bedrock-core (Go)
                                                        ├─ RakNet ──── servers
                                                        └─ NetherNet ─ Realms and friend worlds
```

The Rust client owns everything you see and play. The Go core owns Xbox sign-in, encryption and
the network transports. The target game version is pinned in `assets/bedrock-target.json`.

Built on these libraries:

| Library | Used for |
| --- | --- |
| [protocolgen](https://github.com/cinnabar-mc/protocolgen) | Generates the Bedrock packet codec vendored in `crates/protocol/vendor`. |
| [Axolotl Stack](https://github.com/axolotl-stack/axolotl-stack) | Jolyne (client session library), whose raw packet entries and spawn sequence `crates/protocol` adapts. MIT; see `THIRD_PARTY_NOTICES.md`. |
| [gophertunnel](https://github.com/Sandertv/gophertunnel) | Bedrock login, encryption, resource packs and the packet relay. |
| [go-raknet](https://github.com/Sandertv/go-raknet) | RakNet transport to servers, plus server-list pings. |
| [go-nethernet](https://github.com/df-mc/go-nethernet) | WebRTC transport for Realms and friend worlds. |
| [go-xsapi](https://github.com/df-mc/go-xsapi) | Xbox Live identity, friends, presence and signaling. |
| [go-playfab](https://github.com/df-mc/go-playfab) | PlayFab sign-in and the menu catalog (featured servers). |
| [dragonfly](https://github.com/df-mc/dragonfly) | Checkout-only server and mod development tools in `tools/localserver`; player worlds use BDS. |

Crate-by-crate layout: [docs/architecture/workspace.md](docs/architecture/workspace.md).
Benchmarks: [docs/benchmarks.md](docs/benchmarks.md).
Roadmap: [plan.md](plan.md). Change history: [CHANGELOG.md](CHANGELOG.md).

## Contributing

Open pull requests against `dev`, the default and release branch. Before pushing, check only what
your change affects:

```sh
cargo run -p devtool --locked -- verify-affected --base origin/dev
```

Contributor rules are in [AGENTS.md](AGENTS.md) and [docs/agents/](docs/agents/).

## Star history

[![Star history chart](https://api.star-history.com/svg?repos=cinnabar-mc/cinnabar&type=Date)](https://star-history.com/#cinnabar-mc/cinnabar&Date)

## License

[PolyForm Noncommercial 1.0.0](LICENSE). Contact the maintainers for a commercial license.
Third-party code and assets keep their own licenses; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
