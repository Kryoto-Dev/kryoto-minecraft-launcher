# Developing Kryoto Minecraft Launcher

The details behind the README's quick start.

## How a version gets from Mojang to a running game

1. **The list.** `meta::manifest` reads Mojang's `version_manifest_v2.json` (cached for an hour,
   and kept for offline use).
2. **Installing** (`install::prepare`, the same code Play runs with quick checks):
   the version file, the client jar, every library the rules allow on this computer, natives,
   Mojang's log4j config, the asset index and objects, and the Java runtime the version names
   (`javaVersion.component`: `jre-legacy` is Java 8, `java-runtime-delta` 21, `-epsilon` 25...),
   all from Mojang. Files already on disk with the right size are skipped; Repair checks SHA-1s.
   Downloads go to `.part` files, are hashed while they stream, and are retried three times.
3. **Loaders.** Fabric and Quilt publish version files that inherit from the vanilla one.
   Forge and NeoForge are installed by their own installers, run headless
   (`--installClient <data folder>`), because from 1.13 they patch the game with processors.
   Forge's oldest installers can't run headless; their version file and jar are taken out of
   the installer.
4. **Starting** (`launch::command_line`): the inheritance chain merged into one version, rules
   evaluated, `${placeholders}` filled in, both argument formats (`minecraftArguments` before
   1.13, `arguments` lists after). Joining a server uses `--quickPlayMultiplayer` (1.20+, which
   reads SRV records) or `--server/--port`.
5. **Watching.** Output is read line by line; Mojang's log4j XML events become plain lines.
   A bad exit is explained from the log (wrong Java, memory, graphics driver, a broken mod).

## Data folder

`%APPDATA%\to.kryo.minecraft` (`to.kryo.minecraft.dev` for debug builds). The layout is the
official launcher's (`versions/`, `libraries/`, `assets/`), plus `runtimes/` for Java and
`instances/<id>/minecraft/` for each instance's game folder. See `crates/kryoto-mc/src/paths.rs`.

## Profiles

A profile is a name. Starting a game without one asks for it first (`src/profiles/require.ts`)
and saves it as the first profile, so nobody plays under a placeholder. Its UUID is the one an offline-mode server computes
(`OfflinePlayer:<name>`), so worlds and server inventories follow the name. There is no
account sign-in: profiles play singleplayer, LAN and offline-mode servers (`online-mode=false`,
as play.kryo.to runs). The game starts with access token `0` and user type `legacy`.

## Baby mobs

`src/mobs/` draws baby mobs walking along the bottom of the window, in three.js, loaded only
when they are shown. Nothing of Mojang's is in the launcher: `crates/kryoto-mc/src/mobs/`
reads each mob from the newest installed client jar at the moment it is shown.

- **Shape:** `model.rs` parses the mob's model class (`BabyPigModel`, `BabyZombieModel`...) and
  runs the method that builds its body layer (the class's own or a parent's) on a small stack
  machine that knows only the model builder calls (`CubeListBuilder`, `PartPose`,
  `PartDefinition`, `LayerDefinition`) and the opcodes such methods use. Anything else stops
  it, and that mob isn't shown.
- **Look:** the mob's texture from the same jar; `mod.rs` lists the mobs, their model classes,
  their textures and which are mean.
- **Movement** is the launcher's own: legs, arms, heads, tails and wings are found by part name.

The mobs need a version with baby models (26.1 and later). Settings, Interface: on or off, how
many at most, and whether the mean ones (baby zombies, drowned, piglins, hoglins) may come.
Reduced motion keeps them standing still. To check every mob against an installed game:

```bash
KRYOTO_TEST_ROOT="$APPDATA/to.kryo.minecraft" \
  cargo test --release --manifest-path crates/kryoto-mc/Cargo.toml --lib mobs -- --include-ignored --nocapture
```

## Linux

`src-tauri/src/linux.rs` sets WebKitGTK's renderer before the window starts (Kryoto Desktop's
findings: DMA-BUF off, XWayland under Wayland), and `installEdgeResize` in `src/lib/window.ts`
gives the frameless window resize edges. Checking the engine's Linux code from Windows, without
a Linux C compiler (nothing is linked, so MinGW's gcc can stand in for ring's C):

```bash
CC_x86_64_unknown_linux_gnu=gcc AR_x86_64_unknown_linux_gnu=ar \
  cargo clippy --target x86_64-unknown-linux-gnu --manifest-path crates/kryoto-mc/Cargo.toml --all-targets -- -D warnings
```

The app itself (GTK) only builds on Linux; CI and the release workflow do that on Ubuntu 22.04.

## Building

```text
pnpm app:build:local   # unsigned, no updater files; copied to D:\kryo-builds\kryoto-minecraft-launcher
                       # (or ~/kryoto-builds/...; set KRYOTO_LOCAL_OUT to change it)
pnpm app:build         # the signed release build; needs TAURI_SIGNING_PRIVATE_KEY
```

Run local installers from outside the repository folder: a program started inside a
workspace folder can be confined to writing inside it.

On a Windows machine with Rust's GNU toolchain (MinGW), the app is a plain `rlib` (a `cdylib`
exceeds MinGW's export limit), and the engine lives in its own crate partly so its tests run:
test binaries that link Tauri need a Common Controls v6 manifest MinGW can't embed.

Run `pnpm` scripts from PowerShell on machines where Git Bash resolves Node through a short
path it cannot start.

## The real test

`crates/kryoto-mc/tests/real.rs` installs versions from Mojang, starts each with an offline
profile, waits for the game to come up, and stops it. It downloads a lot and opens game
windows, so it only runs when asked:

```bash
KRYOTO_TEST_ROOT=D:/kryo-test \
KRYOTO_TEST_GAMES="latest vanilla; 1.21.1 fabric; 1.21.1 neoforge; 1.12.2 forge; b1.7.3 vanilla" \
  cargo test --manifest-path crates/kryoto-mc/Cargo.toml --test real -- --ignored --nocapture
```

The Fabric entry also adds Sodium from Modrinth with its dependencies.

## Releasing

See the README. The updater's key pair is the launcher's own: the private key goes in the
repository secret `TAURI_SIGNING_PRIVATE_KEY` (its password, if any, in
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), the public half is `plugins.updater.pubkey` in
`tauri.conf.json`. Losing the private key means installed copies can't update past that point.

Keep `@tauri-apps/api` and `@tauri-apps/cli` on the same minor version as the `tauri` crate.

## Shared files

Files listed in `shared.json` are committed copies from `kryoto-desktop/src` (the UI
primitives, the K// mark, the native bridge), as Kryoto Mobile keeps them. Change them in
Desktop, then `pnpm sync:shared` here; `pnpm build` fails if a copy was edited in this repo.
`src/styles.css` is Desktop's with its Desktop-only parts removed.
