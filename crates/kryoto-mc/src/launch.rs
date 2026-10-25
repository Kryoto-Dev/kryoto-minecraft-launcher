//! Starting the game: the command line, the running process, its log, and
//! what to tell the player when it stops badly.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::auth;
use crate::install::Prepared;
use crate::meta::{self, Argument, Features, StringOrList};
use crate::paths::Paths;
use crate::tasks::now;

pub const LAUNCHER_NAME: &str = "kryoto";

/// How a game is started, beyond what the instance says.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    /// Join this server straight away (`host` or `host:port`).
    pub server: Option<String>,
    /// Open this world straight away (its folder name), 1.20 and later.
    pub world: Option<String>,
}

pub struct CommandLine {
    pub java: PathBuf,
    pub args: Vec<String>,
    /// For the log: the same, as one line.
    pub shown: String,
}

pub struct Settings<'a> {
    pub memory_mb: u32,
    pub jvm_args: &'a str,
    pub resolution: Option<(u32, u32)>,
    pub fullscreen: bool,
}

/// Build the java command line for a prepared version.
pub fn command_line(paths: &Paths, prepared: &Prepared, game_dir: &Path, natives_dir: &Path, profile: &auth::Profile, settings: &Settings, extras: &Extras) -> CommandLine {
    let json = &prepared.resolved.json;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut classpath: Vec<String> = prepared.classpath.iter().map(|p| p.display().to_string()).collect();
    classpath.push(prepared.client_jar.display().to_string());

    // An address without a port is passed as it is to versions with quick
    // play, which then look up the server's SRV record (play.kryo.to has one).
    // Older versions' --server needs a port; they never read SRV records.
    let server = extras.server.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (server_host, server_port) = match server {
        Some(s) => match s.rsplit_once(':') {
            Some((h, p)) if p.parse::<u16>().is_ok() => (Some(h.to_string()), Some(p.to_string())),
            _ => (Some(s.to_string()), Some("25565".to_string())),
        },
        None => (None, None),
    };
    let quick_play_multiplayer = server.map(str::to_string);

    let mut features = Features::new();
    features.insert("has_custom_resolution".into(), settings.resolution.is_some());
    features.insert("is_demo_user".into(), false);
    features.insert("is_quick_play_multiplayer".into(), quick_play_multiplayer.is_some());
    features.insert("is_quick_play_singleplayer".into(), extras.world.is_some());

    let (width, height) = settings.resolution.unwrap_or((854, 480));
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", profile.name.clone()),
        ("version_name", json.id.clone()),
        ("game_directory", game_dir.display().to_string()),
        ("assets_root", paths.assets().display().to_string()),
        ("game_assets", prepared.game_assets.display().to_string()),
        ("assets_index_name", prepared.assets_index.clone()),
        ("auth_uuid", profile.id.clone()),
        // No account, so no token: "0" is what offline launchers have always passed.
        ("auth_access_token", "0".into()),
        ("auth_session", "-".into()),
        ("auth_xuid", "0".into()),
        ("clientid", String::new()),
        ("user_type", "legacy".into()),
        ("user_properties", "{}".into()),
        ("version_type", json.kind.clone().unwrap_or_else(|| "release".into())),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
        ("natives_directory", natives_dir.display().to_string()),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", classpath.join(sep)),
        ("classpath_separator", sep.into()),
        ("library_directory", paths.libraries().display().to_string()),
        ("quickPlayPath", game_dir.join("quickPlay").join("log.json").display().to_string()),
        ("quickPlayMultiplayer", quick_play_multiplayer.clone().unwrap_or_default()),
        ("quickPlaySingleplayer", extras.world.clone().unwrap_or_default()),
        ("quickPlayRealms", String::new()),
    ]);
    let fill = |s: &str| substitute(s, &vars);

    let mut args: Vec<String> = vec![format!("-Xmx{}M", settings.memory_mb), format!("-Xms{}M", settings.memory_mb.min(1024))];
    // The official launcher's garbage collector settings: shorter pauses.
    if prepared.java_major >= 8 {
        args.extend(
            ["-XX:+UnlockExperimentalVMOptions", "-XX:+UseG1GC", "-XX:G1NewSizePercent=20", "-XX:G1ReservePercent=20", "-XX:MaxGCPauseMillis=50", "-XX:G1HeapRegionSize=32M"]
                .map(String::from),
        );
    }
    args.extend(split_args(settings.jvm_args));

    match json.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(arguments) => args.extend(evaluate(&arguments.jvm, &features).iter().map(|a| fill(a))),
        None => {
            if cfg!(windows) {
                args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            }
            args.push(fill("-Djava.library.path=${natives_directory}"));
            args.push(fill("-Dminecraft.launcher.brand=${launcher_name}"));
            args.push(fill("-Dminecraft.launcher.version=${launcher_version}"));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    if let Some(logging) = &prepared.logging_arg {
        args.push(logging.clone());
    }
    args.push(json.main_class.clone().unwrap_or_else(|| "net.minecraft.client.main.Main".into()));

    let mut game: Vec<String> = match (&json.arguments, &json.minecraft_arguments) {
        (Some(arguments), _) if !arguments.game.is_empty() => evaluate(&arguments.game, &features).iter().map(|a| fill(a)).collect(),
        (_, Some(legacy)) => legacy.split_whitespace().map(&fill).collect(),
        _ => Vec::new(),
    };
    let modern = json.arguments.as_ref().is_some_and(|a| !a.game.is_empty());
    // Before 1.13 the version file has no resolution or quick-play rules; the
    // game still takes the options from 1.6 on (the `--username` style).
    let options_style = game.iter().any(|a| a == "--username");
    if !modern && options_style {
        if settings.resolution.is_some() {
            game.extend(["--width".into(), width.to_string(), "--height".into(), height.to_string()]);
        }
        if let (Some(host), Some(port)) = (&server_host, &server_port) {
            game.extend(["--server".into(), host.clone(), "--port".into(), port.clone()]);
        }
    }
    if modern && quick_play_multiplayer.is_some() && !game.iter().any(|a| a == "--quickPlayMultiplayer") {
        // Versions from 1.6 to 1.19 that use argument lists still know --server.
        if let (Some(host), Some(port)) = (&server_host, &server_port) {
            game.extend(["--server".into(), host.clone(), "--port".into(), port.clone()]);
        }
    }
    if settings.fullscreen && (modern || options_style) {
        game.push("--fullscreen".into());
    }
    args.extend(game);

    let shown = std::iter::once(prepared.java.display().to_string())
        .chain(args.iter().cloned())
        .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a })
        .collect::<Vec<_>>()
        .join(" ");
    CommandLine { java: prepared.java.clone(), args, shown }
}

fn evaluate(list: &[Argument], features: &Features) -> Vec<String> {
    let mut out = Vec::new();
    for arg in list {
        match arg {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Ruled { rules, value } => {
                if meta::allowed(Some(rules), features) {
                    match value {
                        StringOrList::One(s) => out.push(s.clone()),
                        StringOrList::Many(v) => out.extend(v.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// Replace `${name}` placeholders; unknown ones are left as they are.
pub fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match vars.get(name) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Split a line of arguments a player typed, honouring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                any = false;
            }
            c => current.push(c),
        }
    }
    if any || !current.is_empty() {
        out.push(current);
    }
    out
}

/* ── Running games ──────────────────────────────────────── */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub line: String,
    /// "out", "warn", "err" or "launcher".
    pub stream: &'static str,
}

/// Mojang's logging config prints log4j XML events (for launchers to read);
/// this turns them back into the lines the game would print to a console.
/// Lines that are not XML (old versions, mods printing directly) pass as they are.
#[derive(Default)]
pub struct Log4j {
    event: Option<String>,
}

impl Log4j {
    pub fn feed(&mut self, line: String, stream: &'static str) -> Vec<LogLine> {
        match self.event.as_mut() {
            None if line.trim_start().starts_with("<log4j:Event") => self.event = Some(line),
            None => return vec![LogLine { line, stream }],
            Some(event) => {
                event.push('\n');
                event.push_str(&line);
            }
        }
        if !self.event.as_deref().is_some_and(|e| e.contains("</log4j:Event>")) {
            return Vec::new();
        }
        let event = self.event.take().unwrap_or_default();
        parse_event(&event)
    }
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

/// The text of every CDATA section inside `element`.
fn cdata(event: &str, element: &str) -> Option<String> {
    let open = format!("<log4j:{element}>");
    let start = event.find(&open)? + open.len();
    let end = event[start..].find(&format!("</log4j:{element}>"))? + start;
    let body = &event[start..end];
    let mut out = String::new();
    let mut rest = body;
    while let Some(a) = rest.find("<![CDATA[") {
        let after = &rest[a + 9..];
        let b = after.find("]]>").unwrap_or(after.len());
        out.push_str(&after[..b]);
        rest = after.get(b + 3..).unwrap_or("");
    }
    Some(out)
}

fn parse_event(event: &str) -> Vec<LogLine> {
    let head = event.lines().next().unwrap_or("");
    let level = attribute(head, "level").unwrap_or("INFO");
    let thread = attribute(head, "thread").unwrap_or("main");
    let time = attribute(head, "timestamp")
        .and_then(|t| t.parse::<i64>().ok())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
        .unwrap_or_default();
    let stream = match level {
        "ERROR" | "FATAL" => "err",
        "WARN" => "warn",
        _ => "out",
    };
    let message = cdata(event, "Message").unwrap_or_default();
    let mut lines: Vec<LogLine> = Vec::new();
    for (i, text) in message.lines().enumerate() {
        let line = if i == 0 { format!("[{time}] [{thread}/{level}]: {text}") } else { text.to_string() };
        lines.push(LogLine { line, stream });
    }
    if lines.is_empty() {
        lines.push(LogLine { line: format!("[{time}] [{thread}/{level}]:"), stream });
    }
    if let Some(throwable) = cdata(event, "Throwable") {
        lines.extend(throwable.lines().filter(|l| !l.is_empty()).map(|l| LogLine { line: l.to_string(), stream }));
    }
    lines
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exit {
    pub code: Option<i32>,
    pub seconds: u64,
    /// What probably went wrong, in words, when it stopped badly.
    pub hint: Option<String>,
    pub crash_report: Option<String>,
    pub stopped_by_player: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub instance: String,
    pub running: bool,
    pub started: Option<u64>,
    pub pid: Option<u32>,
    pub exit: Option<Exit>,
}

#[derive(Default)]
struct Game {
    started: u64,
    pid: Option<u32>,
    kill: Option<tokio::sync::oneshot::Sender<()>>,
    log: VecDeque<LogLine>,
    last_exit: Option<Exit>,
}

/// Every game this launcher started, and their logs.
#[derive(Clone, Default)]
pub struct Games {
    inner: Arc<Mutex<HashMap<String, Game>>>,
}

const LOG_LINES: usize = 5000;

pub type LogEmit = Arc<dyn Fn(&str, &[LogLine]) + Send + Sync>;
pub type StateEmit = Arc<dyn Fn(&GameState) + Send + Sync>;

impl Games {
    pub fn running(&self, instance: &str) -> bool {
        self.inner.lock().unwrap().get(instance).is_some_and(|g| g.pid.is_some())
    }

    pub fn states(&self) -> Vec<GameState> {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .map(|(id, g)| GameState { instance: id.clone(), running: g.pid.is_some(), started: g.pid.map(|_| g.started), pid: g.pid, exit: g.last_exit.clone() })
            .collect()
    }

    pub fn log(&self, instance: &str) -> Vec<LogLine> {
        self.inner.lock().unwrap().get(instance).map(|g| g.log.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn note(&self, instance: &str, line: impl Into<String>, emit: &LogEmit) {
        let line = LogLine { line: line.into(), stream: "launcher" };
        self.push(instance, vec![line.clone()]);
        emit(instance, &[line]);
    }

    /// A fresh log for a new start.
    pub fn reset_log(&self, instance: &str) {
        let mut all = self.inner.lock().unwrap();
        let game = all.entry(instance.to_string()).or_default();
        game.log.clear();
        game.last_exit = None;
    }

    fn push(&self, instance: &str, lines: Vec<LogLine>) {
        let mut all = self.inner.lock().unwrap();
        let game = all.entry(instance.to_string()).or_default();
        for l in lines {
            game.log.push_back(l);
        }
        while game.log.len() > LOG_LINES {
            game.log.pop_front();
        }
    }

    pub fn stop(&self, instance: &str) -> bool {
        let mut all = self.inner.lock().unwrap();
        match all.get_mut(instance).and_then(|g| g.kill.take()) {
            Some(kill) => kill.send(()).is_ok(),
            None => false,
        }
    }

    /// Start the process and watch it. `on_exit` runs once it has stopped.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &self,
        instance: &str,
        command: &CommandLine,
        game_dir: &Path,
        log_file: PathBuf,
        emit_log: LogEmit,
        emit_state: StateEmit,
        on_exit: impl FnOnce(&Exit) + Send + 'static,
    ) -> Result<u32, String> {
        let mut cmd = tokio::process::Command::new(&command.java);
        cmd.args(&command.args).current_dir(game_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::java::no_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Java ({}): {e}", command.java.display()))?;
        let pid = child.id().unwrap_or(0);
        let started = now();
        let (kill_tx, mut kill_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut all = self.inner.lock().unwrap();
            let game = all.entry(instance.to_string()).or_default();
            game.started = started;
            game.pid = Some(pid);
            game.kill = Some(kill_tx);
            game.last_exit = None;
        }
        emit_state(&GameState { instance: instance.into(), running: true, started: Some(started), pid: Some(pid), exit: None });

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<LogLine>();
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        for (stream, reader) in [("out", Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), ("err", Box::new(stderr))] {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(reader).lines();
                let mut log4j = Log4j::default();
                while let Ok(Some(line)) = lines.next_line().await {
                    for parsed in log4j.feed(line, stream) {
                        if tx.send(parsed).is_err() {
                            return;
                        }
                    }
                }
            });
        }
        drop(tx);

        // Lines go to the UI in batches, and to the launcher's copy of the log.
        let games = self.clone();
        let id = instance.to_string();
        let forward = {
            let games = games.clone();
            let id = id.clone();
            let emit_log = emit_log.clone();
            tokio::spawn(async move {
                let mut file = std::fs::File::create(&log_file).ok();
                let mut batch = Vec::new();
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(120));
                loop {
                    tokio::select! {
                        line = rx.recv() => match line {
                            Some(l) => {
                                if let Some(f) = file.as_mut() {
                                    use std::io::Write;
                                    let _ = writeln!(f, "{}", l.line);
                                }
                                batch.push(l);
                                if batch.len() >= 400 {
                                    games.push(&id, batch.clone());
                                    emit_log(&id, &batch);
                                    batch.clear();
                                }
                            }
                            None => break,
                        },
                        _ = tick.tick() => if !batch.is_empty() {
                            games.push(&id, batch.clone());
                            emit_log(&id, &batch);
                            batch.clear();
                        }
                    }
                }
                if !batch.is_empty() {
                    games.push(&id, batch.clone());
                    emit_log(&id, &batch);
                }
            })
        };

        let game_dir = game_dir.to_path_buf();
        tokio::spawn(async move {
            let mut stopped_by_player = false;
            let status = tokio::select! {
                status = child.wait() => status,
                _ = &mut kill_rx => {
                    stopped_by_player = true;
                    let _ = child.kill().await;
                    child.wait().await
                }
            };
            let _ = forward.await;
            let code = status.ok().and_then(|s| s.code());
            let mut log = games.log(&id);
            let crash_report = newest_crash_report(&game_dir, started);
            // The game doesn't always print the report it saved; its first
            // lines name the cause.
            if let Some(text) = crash_report.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
                log.extend(text.lines().take(120).map(|l| LogLine { line: l.to_string(), stream: "err" }));
            }
            let hint = if stopped_by_player || code == Some(0) { None } else { Some(explain_exit(code, &log, crash_report.is_some())) };
            let exit = Exit { code, seconds: now().saturating_sub(started), hint, crash_report: crash_report.map(|p| p.display().to_string()), stopped_by_player };
            {
                let mut all = games.inner.lock().unwrap();
                if let Some(g) = all.get_mut(&id) {
                    g.pid = None;
                    g.kill = None;
                    g.last_exit = Some(exit.clone());
                }
            }
            on_exit(&exit);
            emit_state(&GameState { instance: id.clone(), running: false, started: None, pid: None, exit: Some(exit) });
        });
        Ok(pid)
    }
}

fn newest_crash_report(game_dir: &Path, since: u64) -> Option<PathBuf> {
    std::fs::read_dir(game_dir.join("crash-reports"))
        .ok()?
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            (modified + 2 >= since).then(|| (modified, e.path()))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// What a bad exit most likely means, from the last part of the log.
pub fn explain_exit(code: Option<i32>, log: &[LogLine], crash_report: bool) -> String {
    let tail: String = log.iter().rev().take(400).map(|l| l.line.as_str()).collect::<Vec<_>>().join("\n");
    let has = |s: &str| tail.contains(s);
    if has("cannot be cast to class java.net.URLClassLoader") || has("cannot be cast to java.net.URLClassLoader") {
        return "This version only runs on Java 8. Set the instance's Java back to Automatic.".into();
    }
    if has("UnsupportedClassVersionError") || has("has been compiled by a more recent version of the Java Runtime") {
        return "This needs a newer Java than the one it was started with. Set the instance's Java back to Automatic, or pick a newer one.".into();
    }
    if has("Could not reserve enough space") || has("Invalid maximum heap size") || has("Initial heap size set to a larger value") {
        return "Java couldn't get the memory it was asked for. Lower the memory for this instance in its settings.".into();
    }
    if has("java.lang.OutOfMemoryError") {
        return "Minecraft ran out of memory. Give this instance more memory in its settings.".into();
    }
    if has("Pixel format not accelerated") || has("WGL: The driver does not appear to support OpenGL") || has("GLFW error 65542") || has("GLFW error 65543") {
        return "Your graphics driver can't run this version's OpenGL. Update the graphics driver; on laptops with two GPUs, run it on the dedicated one.".into();
    }
    if has("Incompatible mods found") || has("Mod resolution failed") || has("FormattedException") || has("Missing or unsupported mandatory dependencies") {
        return "A mod doesn't fit this Minecraft version, this loader or another mod. The log names which one.".into();
    }
    // A mod's patches no longer match the game the loader built: the mod was
    // made for another build of the loader.
    if has("MixinApplyError") || has("InvalidInjectionException") || has("MixinTransformerError") {
        let failing = regex::Regex::new(r"Mixin \[[^\]]*? from mod ([A-Za-z0-9_.-]+)\][^\n]*FAILED")
            .ok()
            .and_then(|re| re.captures(&tail).map(|c| c[1].to_string()));
        return match failing {
            Some(m) => format!("The mod \"{m}\" doesn't fit this build of the mod loader. Add it again from Modrinth to get its newest file, or remove it."),
            None => "A mod doesn't fit this build of the mod loader. The log names which one; add it again from Modrinth to get its newest file, or remove it.".into(),
        };
    }
    if has("ModLoadingException") || has("LoadingFailedException") || has("Failed to load mod") {
        return "A mod failed to load. The log names which one; remove or update it.".into();
    }
    if has("Could not find or load main class") || has("ClassNotFoundException: net.minecraft") {
        return "Some of the game's files are missing. Use Repair in the instance's settings.".into();
    }
    if has("Error: could not open") && has(".jar") {
        return "A game file couldn't be opened. Use Repair in the instance's settings.".into();
    }
    let report = if crash_report { " A crash report was saved; open it from the log." } else { "" };
    match code {
        Some(code) => format!("Minecraft closed with an error (exit code {code}). The log above says why.{report}"),
        None => format!("Minecraft was stopped by the system.{report}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        let vars = HashMap::from([("a", "1".to_string()), ("b", "two".to_string())]);
        assert_eq!(substitute("x${a}y${b}z", &vars), "x1ytwoz");
        assert_eq!(substitute("${unknown} ${a}", &vars), "${unknown} 1");
        assert_eq!(substitute("open ${a", &vars), "open ${a");
    }

    #[test]
    fn typed_arguments() {
        assert_eq!(split_args(r#"-Dfoo=1  "-Dbar=two words" -Xss2M"#), ["-Dfoo=1", "-Dbar=two words", "-Xss2M"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn log4j_events() {
        let mut p = Log4j::default();
        assert!(p.feed(r#"<log4j:Event logger="x" timestamp="1700000000000" level="WARN" thread="Render thread">"#.into(), "out").is_empty());
        assert!(p.feed("  <log4j:Message><![CDATA[Hello".into(), "out").is_empty());
        assert!(p.feed("world]]></log4j:Message>".into(), "out").is_empty());
        let lines = p.feed("</log4j:Event>".into(), "out");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].line.ends_with("[Render thread/WARN]: Hello"));
        assert_eq!(lines[0].stream, "warn");
        assert_eq!(lines[1].line, "world");
        assert_eq!(p.feed("plain".into(), "err")[0].line, "plain");
    }

    #[test]
    fn exit_hints() {
        let line = |s: &str| LogLine { line: s.into(), stream: "err" };
        assert!(explain_exit(Some(1), &[line("java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main")], false).contains("newer Java"));
        assert!(explain_exit(Some(1), &[line("Error occurred during initialization of VM"), line("Could not reserve enough space for 8388608KB object heap")], false).contains("memory"));
        assert!(explain_exit(Some(-1), &[], true).contains("crash report"));
        let mixin = line("Caused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [sodium-neoforge.mixins.json:platform.neoforge.LevelExtractorMixin from mod sodium] from phase [DEFAULT] in config [sodium-neoforge.mixins.json] FAILED during APPLY");
        assert!(explain_exit(Some(1), &[mixin], true).contains("\"sodium\""));
    }
}
