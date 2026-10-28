# Changelog

## 0.1.4

- CurseForge: browse and add mods, resource packs and shader packs from CurseForge beside
  Modrinth, with the mods they require, and import CurseForge modpacks (`.zip`) from Import a
  modpack or by dropping them on the window. A file whose author only allows downloads on
  CurseForge's own site comes from Modrinth when Modrinth has the very same file; otherwise
  it is named with its page, to fetch there and drop in. Thanks to HallowMarch for asking.
- Java arguments that pick a garbage collector (`-XX:+UseZGC`, the field's own example) start
  the game: the launcher's G1 settings stood beside them and Java refused to start ("Multiple
  garbage collectors selected").
- When Java won't start with the Java arguments, the reason says which argument and where to
  change it, and a stop no longer says "the log above" when the log is below.
- A game that stops badly shows one notice, not two, with Show the log to open its log.
- The command line in the log names how many class path files there are instead of listing
  hundreds of them over the lines that say what went wrong.
- Quilt offers its newest loader: its list came in no order, so 26.3 was offered 0.24.0
  instead of 0.30.1.
- Forge is only offered where it has an installer: Minecraft 1.1 to 1.5.1, and some 1.5.2 to
  1.6.4 builds, ended in "Forge has no installer for this version".
- Each loader says what it is for when creating an instance.

## 0.1.3

- The window shows on new Linux distros: with the .rpm on Fedora 44 and others it was
  see-through apart from its text and buttons. Thanks to [paranoid.K//](https://kryo.to/user/paranoid.kryo.to) for reporting it.
- The window's corners follow your corner style (sharp to pill) on Windows 10, Windows 11 and
  Linux, and are square while maximized or full screen.
- The folder buttons open your file manager on Linux, from the AppImage and on KDE too, where
  they did nothing. Thanks to [paranoid.K//](https://kryo.to/user/paranoid.kryo.to) for reporting it.

## 0.1.2

- Import a Modrinth modpack (`.mrpack`): Import a modpack on the home page, or drop the file
  on the window. It becomes an instance with the pack's Minecraft version, loader, mods and
  configs, the mods named from Modrinth; a pack that would put files outside its instance or
  download from somewhere Modrinth packs may not is refused.
- Mods from Modrinth come in their newest release or beta, not an older release: Sodium
  0.9.2 crashed NeoForge 26.3.0.99 on start, and 0.9.3-beta.1 runs. Add a mod again to get
  the newer file.
- A mod whose patches no longer fit the loader's build is named when the game stops, with
  what to do about it, also when only the crash report says so.

## 0.1.1

- A new icon: a grass block with the K// on its front, so the launcher is easy to tell apart
  from Kryoto Desktop and Forge on the taskbar and desktop.

## 0.1.0

The first version.

- Every Minecraft: Java Edition version Mojang lists: releases, snapshots, Beta and Alpha.
- Game files, libraries, assets and the right Java for each version download from Mojang by
  themselves; files already there are never downloaded twice, failed files are retried, Repair
  checks everything against Mojang's checksums.
- Instances, each with its own worlds, mods, packs and launch settings.
- Fabric, Quilt, Forge and NeoForge.
- Mods, resource packs and shader packs from Modrinth, with the mods they require; drop files on
  the window to add them; turn mods on and off.
- Profiles: pick a name and play singleplayer, LAN and offline-mode servers. No account needed;
  the first Play asks for the name.
- Join a server straight from the launcher, Kryo.to Anarchy first.
- The game's log in the launcher, and a plain-words reason when it stops badly.
- Baby mobs walk along the bottom of the window in 3D: chicks, piglets, calves, puppies,
  kittens, foals and more, read from the game you installed, a different mix every start.
  Choose how many, and whether baby zombies and other mean ones may come. Click one.
- Windows and Linux (AppImage, .deb, .rpm).
- Updates itself.
