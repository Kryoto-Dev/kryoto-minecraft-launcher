//! Baby mobs for the launcher's decoration: they walk along the bottom of the
//! window, read from the player's own installed game.
//!
//! The launcher ships no Mojang files. Each mob's shape is read from its
//! model class in a client jar this player downloaded from Mojang
//! (`model.rs`), and its look from that jar's texture, at the moment it is
//! shown. Until a version with baby mobs (26.1 and later) is installed, there
//! are none.

mod model;

use std::io::Read;
use std::path::PathBuf;

use base64::Engine;
use serde::Serialize;

pub use model::{Cube, Model, Part};

use crate::paths::Paths;

/// How a mob gets about.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gait {
    Walk,
    Hop,
}

struct Kind {
    id: &'static str,
    name: &'static str,
    class: &'static str,
    /// Under `assets/minecraft/textures/entity/`, without `.png`.
    textures: &'static [&'static str],
    mean: bool,
    gait: Gait,
}

const fn kind(id: &'static str, name: &'static str, class: &'static str, textures: &'static [&'static str], mean: bool, gait: Gait) -> Kind {
    Kind { id, name, class, textures, mean, gait }
}

use Gait::{Hop, Walk};

const KINDS: &[Kind] = &[
    kind("chicken", "Chick", "animal/chicken/BabyChickenModel", &["chicken/chicken_temperate_baby", "chicken/chicken_warm_baby", "chicken/chicken_cold_baby"], false, Walk),
    kind("pig", "Piglet", "animal/pig/BabyPigModel", &["pig/pig_temperate_baby", "pig/pig_warm_baby", "pig/pig_cold_baby"], false, Walk),
    kind("cow", "Calf", "animal/cow/BabyCowModel", &["cow/cow_temperate_baby", "cow/cow_warm_baby", "cow/cow_cold_baby", "cow/mooshroom_red_baby", "cow/mooshroom_brown_baby"], false, Walk),
    kind("sheep", "Lamb", "animal/sheep/BabySheepModel", &["sheep/sheep_baby"], false, Walk),
    kind("wolf", "Puppy", "animal/wolf/BabyWolfModel", &["wolf/wolf_baby", "wolf/wolf_ashen_baby", "wolf/wolf_black_baby", "wolf/wolf_chestnut_baby", "wolf/wolf_rusty_baby", "wolf/wolf_snowy_baby", "wolf/wolf_spotted_baby", "wolf/wolf_striped_baby", "wolf/wolf_woods_baby"], false, Walk),
    kind("cat", "Kitten", "animal/feline/BabyCatModel", &["cat/cat_tabby_baby", "cat/cat_black_baby", "cat/cat_red_baby", "cat/cat_siamese_baby", "cat/cat_british_shorthair_baby", "cat/cat_calico_baby", "cat/cat_persian_baby", "cat/cat_ragdoll_baby", "cat/cat_white_baby", "cat/cat_jellie_baby", "cat/cat_all_black_baby"], false, Walk),
    kind("ocelot", "Ocelot kitten", "animal/feline/BabyOcelotModel", &["cat/ocelot_baby"], false, Walk),
    kind("fox", "Fox kit", "animal/fox/BabyFoxModel", &["fox/fox_baby", "fox/fox_snow_baby"], false, Walk),
    kind("rabbit", "Bunny", "animal/rabbit/BabyRabbitModel", &["rabbit/rabbit_brown_baby", "rabbit/rabbit_white_baby", "rabbit/rabbit_black_baby", "rabbit/rabbit_gold_baby", "rabbit/rabbit_salt_baby", "rabbit/rabbit_white_splotched_baby", "rabbit/rabbit_toast_baby"], false, Hop),
    kind("goat", "Kid", "animal/goat/BabyGoatModel", &["goat/goat_baby"], false, Walk),
    kind("llama", "Cria", "animal/llama/BabyLlamaModel", &["llama/llama_creamy_baby", "llama/llama_white_baby", "llama/llama_brown_baby", "llama/llama_gray_baby"], false, Walk),
    kind("panda", "Panda cub", "animal/panda/BabyPandaModel", &["panda/panda_baby", "panda/brown_panda_baby", "panda/playful_panda_baby", "panda/lazy_panda_baby", "panda/worried_panda_baby"], false, Walk),
    kind("polar_bear", "Polar bear cub", "animal/polarbear/BabyPolarBearModel", &["bear/polarbear_baby"], false, Walk),
    kind("armadillo", "Armadillo pup", "animal/armadillo/BabyArmadilloModel", &["armadillo/armadillo_baby"], false, Walk),
    kind("horse", "Foal", "animal/equine/BabyHorseModel", &["horse/horse_white_baby", "horse/horse_creamy_baby", "horse/horse_chestnut_baby", "horse/horse_brown_baby", "horse/horse_black_baby", "horse/horse_gray_baby", "horse/horse_darkbrown_baby"], false, Walk),
    kind("donkey", "Donkey foal", "animal/equine/BabyDonkeyModel", &["horse/donkey_baby", "horse/mule_baby"], false, Walk),
    kind("turtle", "Hatchling", "animal/turtle/BabyTurtleModel", &["turtle/turtle_baby"], false, Walk),
    kind("axolotl", "Axolotl baby", "animal/axolotl/BabyAxolotlModel", &["axolotl/axolotl_lucy_baby", "axolotl/axolotl_wild_baby", "axolotl/axolotl_gold_baby", "axolotl/axolotl_cyan_baby", "axolotl/axolotl_blue_baby"], false, Walk),
    kind("camel", "Camel calf", "animal/camel/BabyCamelModel", &["camel/camel_baby"], false, Walk),
    kind("villager", "Villager child", "npc/BabyVillagerModel", &["villager/villager_baby"], false, Walk),
    kind("strider", "Strider baby", "monster/strider/BabyStriderModel", &["strider/strider_baby", "strider/strider_cold_baby"], false, Walk),
    kind("zombie", "Baby zombie", "monster/zombie/BabyZombieModel", &["zombie/zombie_baby", "zombie/husk_baby"], true, Walk),
    kind("drowned", "Baby drowned", "monster/zombie/BabyDrownedModel", &["zombie/drowned_baby"], true, Walk),
    kind("zombie_villager", "Baby zombie villager", "monster/zombie/BabyZombieVillagerModel", &["zombie_villager/zombie_villager_baby"], true, Walk),
    kind("piglin", "Baby piglin", "monster/piglin/BabyPiglinModel", &["piglin/piglin_baby"], true, Walk),
    kind("zombified_piglin", "Baby zombified piglin", "monster/piglin/BabyZombifiedPiglinModel", &["piglin/zombified_piglin_baby"], true, Walk),
    kind("hoglin", "Hoglet", "monster/hoglin/BabyHoglinModel", &["hoglin/hoglin_baby", "hoglin/zoglin_baby"], true, Walk),
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub mean: bool,
    pub gait: Gait,
    pub variants: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mob {
    pub id: &'static str,
    pub name: &'static str,
    pub mean: bool,
    pub gait: Gait,
    pub model: Model,
    /// A data: URL of the texture.
    pub texture: String,
}

const MARKER: &str = "net/minecraft/client/model/animal/chicken/BabyChickenModel.class";

/// The newest installed client jar that has baby mob models.
fn newest_jar(paths: &Paths) -> Option<PathBuf> {
    let mut jars: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(paths.versions())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let jar = paths.version_jar(&e.file_name().to_string_lossy());
            Some((jar.metadata().ok()?.modified().ok()?, jar))
        })
        .collect();
    jars.sort_by_key(|j| std::cmp::Reverse(j.0));
    jars.into_iter().map(|(_, j)| j).find(|jar| {
        std::fs::File::open(jar).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some_and(|mut z| z.by_name(MARKER).is_ok())
    })
}

fn open(paths: &Paths) -> Result<zip::ZipArchive<std::fs::File>, String> {
    let jar = newest_jar(paths).ok_or("No installed version has baby mobs yet.")?;
    zip::ZipArchive::new(std::fs::File::open(&jar).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn texture_path(t: &str) -> String {
    format!("assets/minecraft/textures/entity/{t}.png")
}

/// The baby mobs the installed game has.
pub fn available(paths: &Paths) -> Result<Vec<MobInfo>, String> {
    let jar = open(paths)?;
    let names: std::collections::HashSet<&str> = jar.file_names().collect();
    Ok(KINDS
        .iter()
        .filter(|k| names.contains(format!("net/minecraft/client/model/{}.class", k.class).as_str()))
        .filter_map(|k| {
            let variants: Vec<String> = k.textures.iter().filter(|t| names.contains(texture_path(t).as_str())).map(|t| t.to_string()).collect();
            (!variants.is_empty()).then_some(MobInfo { id: k.id, name: k.name, mean: k.mean, gait: k.gait, variants })
        })
        .collect())
}

/// Mobs, each with its model read from the game and one of its textures.
/// One pass over the jar for all of them.
pub fn load(paths: &Paths, wanted: &[(String, String)]) -> Result<Vec<Mob>, String> {
    let mut jar = open(paths)?;
    let mut out = Vec::new();
    for (id, variant) in wanted {
        let kind = KINDS.iter().find(|k| k.id == id).ok_or_else(|| format!("No mob called {id}."))?;
        if !kind.textures.contains(&variant.as_str()) {
            return Err(format!("{} has no look called {variant}.", kind.name));
        }
        let mut model = model::read(&mut jar, &format!("net/minecraft/client/model/{}", kind.class))?;
        let mut entry = jar.by_name(&texture_path(variant)).map_err(|_| format!("This game has no {variant} texture."))?;
        if entry.size() > 256 * 1024 {
            return Err("That texture is too big.".into());
        }
        let mut png = Vec::new();
        entry.read_to_end(&mut png).map_err(|e| e.to_string())?;
        drop(entry);
        if model.texture_width == 0 && png.len() > 24 {
            model.texture_width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
            model.texture_height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        }
        out.push(Mob {
            id: kind.id,
            name: kind.name,
            mean: kind.mean,
            gait: kind.gait,
            model,
            texture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_mobs_and_looks_are_refused() {
        let paths = Paths::new(std::env::temp_dir().join("kryoto-mobs-none"));
        let one = |id: &str, v: &str| load(&paths, &[(id.to_string(), v.to_string())]);
        assert!(one("../../secrets", "x").is_err());
        assert!(one("chicken", "../../../etc/passwd").is_err());
        assert!(available(&paths).unwrap_err().contains("No installed version"));
    }

    /// Reads every mob from a real installed game: `KRYOTO_TEST_ROOT=<data folder> cargo test -- --ignored mobs`.
    #[test]
    #[ignore = "needs an installed game"]
    fn every_mob_reads_from_the_game() {
        let root = std::env::var("KRYOTO_TEST_ROOT").expect("KRYOTO_TEST_ROOT");
        let paths = Paths::new(root.into());
        let mut failed = Vec::new();
        for info in available(&paths).unwrap() {
            match load(&paths, &[(info.id.to_string(), info.variants[0].clone())]) {
                Ok(mobs) => {
                    fn count(p: &Part) -> usize {
                        p.cubes.len() + p.children.iter().map(count).sum::<usize>()
                    }
                    let mob = &mobs[0];
                    println!("{:18} {:3} cubes, texture {}x{}", info.id, count(&mob.model.root), mob.model.texture_width, mob.model.texture_height);
                }
                Err(e) => failed.push(format!("{}: {e}", info.id)),
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }
}
