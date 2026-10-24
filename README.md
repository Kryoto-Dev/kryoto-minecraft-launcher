<p align="center"><img src="docs/logo.png" width="80" alt="Kryoto Minecraft Launcher"></p>

# Kryoto Minecraft Launcher

Install and play any Minecraft: Java Edition version, from today's release back to the first
alpha, with Fabric, Quilt, Forge or NeoForge, mods and packs from Modrinth, and a Microsoft
account or an offline profile. Windows now; the engine already builds on Linux.

**Kryoto** · Tauri 2 (Rust) + React · MIT · [Download](https://github.com/Kryoto-Dev/kryoto-minecraft-launcher/releases/latest) · [All repos](https://github.com/Kryoto-Dev)

The installer holds no Mojang files. Minecraft and the Java it runs on download from Mojang's
own servers when a player installs a version; loaders come from their makers, mods and packs
from Modrinth.

## Quick start

Install Node 22+, pnpm 10 and Rust, then:

```bash
pnpm install
pnpm dev               # the UI alone in a browser, on sample data (http://localhost:1425)
pnpm app:dev           # the app
pnpm app:build:local   # an unsigned installer for this PC, copied out of the repo
```

Debug builds keep their data in `to.kryo.minecraft.dev`, beside an installed copy's.

## Layout

| Path | What's there |
|---|---|
| `crates/kryoto-mc/` | the engine, with no UI: versions, downloads, Java, loaders, mods, accounts, launching |
| `src-tauri/` | the app around it: window, commands, events, file dialogs, updater |
| `src/` | the UI: `library` (instances), `versions`, `downloads`, `settings`, `accounts`, `shell` |
| `src/ui`, `src/lib` (listed in `shared.json`) | copies of Kryoto Desktop's components, kept in step by `pnpm sync:shared` |
| `scripts/` | build, brand art, shared-file sync |
| `docs/` | [development guide](docs/development.md), [Microsoft sign-in](docs/microsoft-sign-in.md) |

## Checks

```bash
pnpm build                                                          # shared files, typecheck, UI build
cargo test --manifest-path crates/kryoto-mc/Cargo.toml              # engine
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
```

The real thing (downloads Minecraft, opens its window, stops it):
[docs/development.md](docs/development.md#the-real-test).

## Release

Bump the version in `package.json`, `src-tauri/Cargo.toml`, `crates/kryoto-mc/Cargo.toml` and
`src-tauri/tauri.conf.json`, add the `CHANGELOG.md` entry and push to `main`. The release
workflow builds, signs and publishes `v<version>`; installed copies update themselves.

## License

[MIT](LICENSE). Minecraft is Mojang's; this project is not affiliated with Mojang or Microsoft.
