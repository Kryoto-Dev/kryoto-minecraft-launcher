//! Installing and starting instances, as one service the app's commands and
//! the integration tests share.

use std::path::PathBuf;
use std::sync::Arc;

use crate::download::Verify;
use crate::install::{self, Options, Prepared};
use crate::instances::{self, Instance, LoaderKind};
use crate::launch::{self, Extras, Games, LogEmit, StateEmit};
use crate::paths::Paths;
use crate::settings;
use crate::tasks::{now, TaskHandle, Tasks};
use crate::{auth, java, loaders};

#[derive(Clone)]
pub struct Core {
    pub paths: Paths,
    pub tasks: Tasks,
    pub games: Games,
    pub emit_log: LogEmit,
    pub emit_state: StateEmit,
    /// Something about an instance changed (installed, played).
    pub emit_instances: Arc<dyn Fn() + Send + Sync>,
}

impl Core {
    /// No UI: for tests and tools.
    pub fn headless(paths: Paths) -> Self {
        Self {
            paths,
            tasks: Tasks::silent(),
            games: Games::default(),
            emit_log: Arc::new(|_, _| {}),
            emit_state: Arc::new(|_| {}),
            emit_instances: Arc::new(|| {}),
        }
    }

    fn parallel(&self) -> usize {
        settings::load(&self.paths).parallel_downloads.clamp(1, 64) as usize
    }

    /// The Java the player chose for this instance (or for all), if any.
    fn chosen_java(&self, instance: &Instance) -> Option<PathBuf> {
        let global = settings::load(&self.paths).java_path;
        instance.overrides.java_path.clone().or(global).filter(|p| !p.trim().is_empty()).map(PathBuf::from)
    }

    /// Refuse a second job on an instance while one runs: two jobs writing
    /// the same files would trip over each other.
    fn idle(&self, id: &str) -> Result<(), String> {
        match self.tasks.running_for(id) {
            Some(title) => Err(format!("Wait for \"{title}\" to finish first.")),
            None => Ok(()),
        }
    }

    /// Install (or check, or repair) everything the instance needs, as a task.
    pub async fn install(&self, id: &str, repair: bool) -> Result<Instance, String> {
        let instance = instances::get(&self.paths, id)?;
        self.idle(id)?;
        let title = if repair { format!("Repairing {}", instance.name) } else { format!("Installing {}", instance.name) };
        let task = self.tasks.start(&title, Some(id), Some(serde_json::json!({ "command": "instance_install", "args": { "id": id, "repair": repair } })))?;
        let result = self.install_in(id, if repair { Verify::Hash } else { Verify::Size }, &task).await;
        task.finish(&result.as_ref().map(|_| ()).map_err(Clone::clone));
        (self.emit_instances)();
        result.map(|(instance, _)| instance)
    }

    async fn install_in(&self, id: &str, verify: Verify, task: &TaskHandle) -> Result<(Instance, Prepared), String> {
        let mut instance = instances::get(&self.paths, id)?;
        let parallel = self.parallel();
        let game_dir = self.paths.game_dir(id);
        let chosen = self.chosen_java(&instance);
        let options = |java: Option<PathBuf>| Options { verify, parallel, java, game_dir: Some(&game_dir) };

        let vanilla = install::prepare(&self.paths, &instance.game_version, options(chosen.clone()), task).await?;
        if instance.version_type.is_empty() {
            instance.version_type = vanilla.resolved.json.kind.clone().unwrap_or_else(|| "release".into());
        }
        let prepared = if instance.loader.kind == LoaderKind::Vanilla {
            instance.launch_version = Some(instance.game_version.clone());
            vanilla
        } else {
            let existing = instance.launch_version.clone().filter(|v| v != &instance.game_version && self.paths.version_json(v).exists());
            let launch_id = match existing {
                Some(id) => id,
                None => {
                    let version = match instance.loader.version.clone() {
                        Some(v) => v,
                        None => {
                            task.stage(&format!("{} versions", instance.loader.kind.label()));
                            let all = loaders::versions(&self.paths, &instance.loader.kind, &instance.game_version).await?;
                            all.iter()
                                .find(|v| v.recommended)
                                .or_else(|| all.first())
                                .map(|v| v.version.clone())
                                .ok_or_else(|| format!("{} has no version for Minecraft {}.", instance.loader.kind.label(), instance.game_version))?
                        }
                    };
                    let launch_id = loaders::install(&self.paths, &instance.loader.kind, &instance.game_version, &version, &vanilla.java, task, parallel).await?;
                    instance.loader.version = Some(version);
                    launch_id
                }
            };
            instance.launch_version = Some(launch_id.clone());
            install::prepare(&self.paths, &launch_id, options(chosen), task).await?
        };
        instance.installed = true;
        instances::save(&self.paths, &instance)?;
        Ok((instance, prepared))
    }

    /// Check the files, sign in, and start the game.
    pub async fn launch(&self, id: &str, extras: Extras) -> Result<u32, String> {
        if self.games.running(id) {
            return Err("This instance is already running.".into());
        }
        let instance = instances::get(&self.paths, id)?;
        self.idle(id)?;
        let task = self.tasks.start(&format!("Starting {}", instance.name), Some(id), Some(serde_json::json!({ "command": "launch", "args": { "id": id } })))?;
        let result = self.launch_in(id, extras, &task).await;
        task.finish(&result.as_ref().map(|_| ()).map_err(Clone::clone));
        (self.emit_instances)();
        result
    }

    async fn launch_in(&self, id: &str, extras: Extras, task: &TaskHandle) -> Result<u32, String> {
        let (instance, prepared) = self.install_in(id, Verify::Size, task).await?;
        let profile = auth::active(&self.paths)?;
        let global = settings::load(&self.paths);
        let o = &instance.overrides;
        let memory_mb = o.memory_mb.unwrap_or(global.memory_mb).max(512);
        let jvm_args = o.jvm_args.clone().unwrap_or_else(|| global.jvm_args.clone());
        let width = o.width.unwrap_or(global.width);
        let height = o.height.unwrap_or(global.height);
        let fullscreen = o.fullscreen.unwrap_or(global.fullscreen);

        self.games.reset_log(id);
        let note = |line: String| self.games.note(id, line, &self.emit_log);
        note(format!("Kryoto Minecraft Launcher {} - {} ({})", env!("CARGO_PKG_VERSION"), instance.name, instance.label()));

        // A Java the player chose must be new enough; one that is newer than
        // an old version wants is allowed, with a warning.
        if self.chosen_java(&instance).is_some() {
            let info = java::probe(&prepared.java).await?;
            if info.major < prepared.java_major {
                return Err(format!(
                    "Minecraft {} needs Java {} or newer, and the Java set for it is Java {}. Set its Java back to Automatic, or pick a newer one.",
                    instance.game_version, prepared.java_major, info.major
                ));
            }
            if prepared.java_major <= 8 && info.major > 8 {
                note(format!("Warning: this version was made for Java 8 and is starting on Java {}. If it crashes, set its Java back to Automatic.", info.major));
            }
        }

        task.stage("Starting");
        let game_dir = self.paths.game_dir(id);
        std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
        let launch_version = instance.launch_version.clone().unwrap_or_else(|| instance.game_version.clone());
        let natives_dir = self.paths.natives(&launch_version);
        install::extract_natives(&prepared.natives, &natives_dir)?;
        let command = launch::command_line(
            &self.paths,
            &prepared,
            &game_dir,
            &natives_dir,
            &profile,
            &launch::Settings { memory_mb, jvm_args: &jvm_args, resolution: Some((width, height)), fullscreen },
            &extras,
        );
        note(format!("Profile: {}", profile.name));
        note(format!("Java: {}", command.java.display()));
        note(command.shown.clone());

        let logs = self.paths.instance(id).join("logs");
        let _ = std::fs::create_dir_all(&logs);
        let paths = self.paths.clone();
        let emit_instances = self.emit_instances.clone();
        let instance_id = id.to_string();
        let started = now();
        let pid = self.games.spawn(id, &command, &game_dir, logs.join("launcher-latest.log"), self.emit_log.clone(), self.emit_state.clone(), move |exit| {
            if let Ok(mut i) = instances::get(&paths, &instance_id) {
                i.last_played = Some(started);
                i.play_seconds += exit.seconds;
                let _ = instances::save(&paths, &i);
            }
            emit_instances();
        })?;
        if let Ok(mut i) = instances::get(&self.paths, id) {
            i.last_played = Some(started);
            let _ = instances::save(&self.paths, &i);
        }
        Ok(pid)
    }
}
