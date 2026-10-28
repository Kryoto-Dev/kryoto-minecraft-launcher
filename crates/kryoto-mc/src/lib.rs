//! The Minecraft side of Kryoto Minecraft Launcher, with no UI in it.
//!
//! | module | what's there |
//! |---|---|
//! | `meta` | Mojang's version list and version files, rules, libraries |
//! | `install` | everything a version needs on disk (jar, libraries, natives, assets, Java) |
//! | `download` | checked, parallel downloads that skip what is already there |
//! | `java` | Mojang's Java runtimes, and probing a player's own Java |
//! | `loaders` | Fabric, Quilt, Forge, NeoForge |
//! | `instances`, `content` | instances and what is in their game folders |
//! | `modrinth`, `curseforge` | finding and installing mods and packs |
//! | `mrpack`, `curseforge` | importing modpacks (`.mrpack`, CurseForge `.zip`) as instances |
//! | `mobs` | baby mobs for the window, read from an installed game |
//! | `auth` | profiles: the names players play under |
//! | `launch`, `core` | the command line, the running game, its log |
//! | `tasks` | progress of every long job, for the UI |

pub mod auth;
pub mod content;
pub mod core;
pub mod curseforge;
pub mod download;
pub mod install;
pub mod instances;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod logging;
pub mod meta;
pub mod mobs;
pub mod modrinth;
pub mod mrpack;
pub mod net;
pub mod paths;
pub mod settings;
pub mod tasks;
