//! Config discovery, parsing, merging, and resolution.
//!
//! Pipeline: [`discover`] picks the file list, [`load`] parses and merges them
//! into one [`Config`], and [`resolve`] expands paths and applies defaults to
//! produce a [`ResolvedConfig`] the planner can consume. [`load_config`] runs all
//! three. See `knowledge/configuration.md`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::{Config, Conflict, LinkValue, MachineMatch, Mapping, Mode, Settings};
use crate::model::{DEFAULT_CONFLICT, DEFAULT_MODE};

/// The machine identity mappings' `os`/`host` conditions are matched against.
/// Plain data, injected like the clock: the binary fills it from the OS, tests
/// pin it, and `symify-core` never reads the environment itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContext {
    /// The running OS, as `std::env::consts::OS` spells it (`linux`, `macos`,
    /// `windows`).
    pub os: String,
    /// The machine's hostname, as reported by the system (undoctored).
    pub host: String,
}

impl MachineContext {
    /// Context with the current OS and the given hostname. The hostname read
    /// is a syscall and lives in the binary, not here.
    pub fn with_host(host: impl Into<String>) -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            host: host.into(),
        }
    }
}

/// Why a mapping is inactive on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveReason {
    /// The `os` condition did not match.
    Os,
    /// The `host` condition did not match.
    Host,
}

impl InactiveReason {
    /// The config key that failed to match, for messages.
    pub fn key(self) -> &'static str {
        match self {
            InactiveReason::Os => "os",
            InactiveReason::Host => "host",
        }
    }
}

/// A fully resolved configuration: every mapping has concrete absolute roots and
/// effective `mode`/`conflict`, ready for planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// Mappings, sorted by name for deterministic output.
    pub mappings: Vec<ResolvedMapping>,
}

/// A single mapping after merge + default application + path expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMapping {
    /// Mapping name (the `[mappings.<name>]` key).
    pub name: String,
    /// Absolute working-location root.
    pub live: PathBuf,
    /// Absolute backing-store root.
    pub store: PathBuf,
    /// Effective link mechanism.
    pub mode: Mode,
    /// Effective conflict policy.
    pub conflict: Conflict,
    /// Link entries, sorted by key for deterministic output.
    pub links: Vec<(String, LinkValue)>,
    /// Set when the mapping's `os`/`host` condition did not match this machine.
    /// An inactive mapping is excluded from planning and status.
    pub inactive: Option<InactiveReason>,
    /// Keep at most this many `.bak` backups per path when writing a new one;
    /// `0` = keep all.
    pub backup_keep: u64,
}

/// Decide which config files to load.
///
/// If `cli_configs` is non-empty it **replaces** default discovery. Otherwise:
/// the default `~/.config/symify/symify.toml` (if present) followed by
/// `~/.config/symify/conf.d/*.toml` in lexicographic order.
pub fn discover(cli_configs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !cli_configs.is_empty() {
        return Ok(cli_configs.to_vec());
    }
    discover_in(&config_base_dir()?)
}

/// Default discovery within a given base directory: `symify.toml` (if present)
/// then `conf.d/*.toml` sorted lexicographically.
fn discover_in(base: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let default = base.join("symify.toml");
    if default.is_file() {
        paths.push(default);
    }

    let conf_d = base.join("conf.d");
    if conf_d.is_dir() {
        let mut drop_ins: Vec<PathBuf> = std::fs::read_dir(&conf_d)
            .map_err(|e| Error::io(&conf_d, e))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        drop_ins.sort();
        paths.extend(drop_ins);
    }

    Ok(paths)
}

/// Parse and merge config files (later files override earlier ones).
///
/// ```no_run
/// use std::path::PathBuf;
/// let merged = symify_core::config::load(&[PathBuf::from("symify.toml")])?;
/// # Ok::<(), symify_core::Error>(())
/// ```
pub fn load(paths: &[PathBuf]) -> Result<Config> {
    let mut merged = Config::default();
    for path in paths {
        let doc = parse_file(path)?;
        merged = merge(merged, doc);
    }
    Ok(merged)
}

/// Discover, load, and resolve in one call.
pub fn load_config(cli_configs: &[PathBuf], machine: &MachineContext) -> Result<ResolvedConfig> {
    let paths = discover(cli_configs)?;
    let config = load(&paths)?;
    resolve(config, machine)
}

/// Result of [`ensure_config`]: the config files to use, and the path of any
/// config that was auto-created so the caller can report it.
#[derive(Debug, Clone)]
pub struct Discovered {
    /// The config files to load (in order).
    pub paths: Vec<PathBuf>,
    /// Set when a default config was just auto-created.
    pub created: Option<PathBuf>,
}

/// Like [`discover`], but in default mode (no `-c`) auto-creates the default
/// config from the starter template when none exists, so every command has
/// something to work with. An explicitly-named (`-c`) but missing file is left
/// to fail later, not auto-created.
pub fn ensure_config(cli_configs: &[PathBuf]) -> Result<Discovered> {
    let paths = discover(cli_configs)?;
    if !paths.is_empty() || !cli_configs.is_empty() {
        return Ok(Discovered {
            paths,
            created: None,
        });
    }

    let path = default_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, render_starter("~", "~/dotfiles")).map_err(|e| Error::io(&path, e))?;
    Ok(Discovered {
        paths: discover(cli_configs)?,
        created: Some(path),
    })
}

/// Restrict a resolved config to the named mappings, preserving order. Empty
/// `names` returns the config unchanged; an unknown name is an error.
pub fn select(config: ResolvedConfig, names: &[String]) -> Result<ResolvedConfig> {
    if names.is_empty() {
        return Ok(config);
    }
    let known: HashSet<&str> = config.mappings.iter().map(|m| m.name.as_str()).collect();
    for name in names {
        if !known.contains(name.as_str()) {
            return Err(Error::config(format!("unknown mapping `{name}`")));
        }
    }
    let want: HashSet<&str> = names.iter().map(String::as_str).collect();
    let mappings = config
        .mappings
        .into_iter()
        .filter(|m| want.contains(m.name.as_str()))
        .collect();
    Ok(ResolvedConfig { mappings })
}

/// The default config path: `$XDG_CONFIG_HOME/symify/symify.toml` or
/// `~/.config/symify/symify.toml`.
pub fn default_config_path() -> Result<PathBuf> {
    Ok(config_base_dir()?.join("symify.toml"))
}

/// URL of the published JSON Schema, embedded in generated configs so editors
/// (taplo / VS Code) can validate and autocomplete them.
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/six5536/symify/main/schema/symify.schema.json";

/// Render an annotated starter `symify.toml` with the given roots. The result is
/// always a valid config that loads to an empty set of links (the examples are
/// commented out).
pub fn render_starter(live: &str, store: &str) -> String {
    format!(
        r#"#:schema {SCHEMA_URL}

# symify configuration.
# Preview with `symify status`, then capture your files with `symify sync`.

[settings]
live = "{live}"          # where your files are used
store = "{store}"        # where the real content is kept (commit this to git)
mode = "symlink"         # symlink | copy (copy = independent copy, kept in sync)
conflict = "backup"      # skip | replace (overwrite, no backup) | backup (.<timestamp>.bak)
# backup_keep = 5        # keep at most N backups per path (absent/0 = keep all)

# Each entry maps a path (relative to `live`) to how it lives in `store`:
#   true / ""   mirror the key under `store`
#   "path"      an explicit path under `store`
#   false       disable the entry
[mappings.dotfiles.links]
# ".bashrc" = true
# ".config/fish/config.fish" = true
"#
    )
}

fn parse_file(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    toml::from_str(&text).map_err(|source| Error::Toml {
        path: path.to_path_buf(),
        source,
    })
}

/// Base config directory: `$XDG_CONFIG_HOME/symify` or `~/.config/symify`.
fn config_base_dir() -> Result<PathBuf> {
    let config_home = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home_dir()?.join(".config"),
    };
    Ok(config_home.join("symify"))
}

/// The user's home directory (Windows-aware via `directories`).
pub(crate) fn home_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or(Error::NoHome)
}

// ----- merge -------------------------------------------------------------

fn merge(base: Config, overlay: Config) -> Config {
    Config {
        settings: merge_settings(base.settings, overlay.settings),
        mappings: merge_mappings(base.mappings, overlay.mappings),
    }
}

fn merge_settings(base: Option<Settings>, overlay: Option<Settings>) -> Option<Settings> {
    match (base, overlay) {
        (None, o) => o,
        (b, None) => b,
        (Some(b), Some(o)) => Some(Settings {
            live: o.live.or(b.live),
            store: o.store.or(b.store),
            mode: o.mode.or(b.mode),
            conflict: o.conflict.or(b.conflict),
            backup_keep: o.backup_keep.or(b.backup_keep),
        }),
    }
}

fn merge_mappings(
    mut base: HashMap<String, Mapping>,
    overlay: HashMap<String, Mapping>,
) -> HashMap<String, Mapping> {
    for (name, mapping) in overlay {
        match base.remove(&name) {
            Some(existing) => base.insert(name, merge_mapping(existing, mapping)),
            None => base.insert(name, mapping),
        };
    }
    base
}

fn merge_mapping(base: Mapping, overlay: Mapping) -> Mapping {
    let mut links = base.links;
    for (k, v) in overlay.links {
        links.insert(k, v);
    }
    Mapping {
        live: overlay.live.or(base.live),
        store: overlay.store.or(base.store),
        mode: overlay.mode.or(base.mode),
        conflict: overlay.conflict.or(base.conflict),
        backup_keep: overlay.backup_keep.or(base.backup_keep),
        os: overlay.os.or(base.os),
        host: overlay.host.or(base.host),
        links,
    }
}

// ----- resolve -----------------------------------------------------------

/// Apply `[settings]` defaults to each mapping, expand `~`/env in roots, make
/// them absolute, evaluate `os`/`host` conditions against `machine`, and sort
/// for determinism.
///
/// ```no_run
/// use symify_core::config::{self, MachineContext};
/// let machine = MachineContext::with_host("wrk-01");
/// let resolved = config::resolve(config::load(&[])?, &machine)?;
/// # Ok::<(), symify_core::Error>(())
/// ```
pub fn resolve(config: Config, machine: &MachineContext) -> Result<ResolvedConfig> {
    let settings = config.settings.unwrap_or_default();
    let home = home_dir().ok();

    let mut names: Vec<String> = config.mappings.keys().cloned().collect();
    names.sort();

    let mut mappings = Vec::with_capacity(names.len());
    for name in names {
        let m = &config.mappings[&name];

        let live = pick_root(m.live.as_deref(), settings.live.as_deref(), &name, "live")?;
        let store = pick_root(
            m.store.as_deref(),
            settings.store.as_deref(),
            &name,
            "store",
        )?;

        let mode = m.mode.or(settings.mode).unwrap_or(DEFAULT_MODE);
        let conflict = m.conflict.or(settings.conflict).unwrap_or(DEFAULT_CONFLICT);
        let backup_keep = m.backup_keep.or(settings.backup_keep).unwrap_or(0);

        let mut links: Vec<(String, LinkValue)> = m
            .links
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        links.sort_by(|a, b| a.0.cmp(&b.0));

        let live = expand_path(&live, home.as_deref())?;
        let store = expand_path(&store, home.as_deref())?;
        if crate::fs::normalize(&live) == crate::fs::normalize(&store) {
            return Err(Error::config(format!(
                "mapping `{name}`: live and store resolve to the same directory ({})",
                live.display()
            )));
        }

        let inactive = machine_mismatch(&name, m, machine)?;

        mappings.push(ResolvedMapping {
            name,
            live,
            store,
            mode,
            conflict,
            links,
            inactive,
            backup_keep,
        });
    }

    Ok(ResolvedConfig { mappings })
}

/// Evaluate a mapping's `os`/`host` conditions against the machine. `None`
/// means active; both conditions must match when both are present.
fn machine_mismatch(
    name: &str,
    m: &Mapping,
    machine: &MachineContext,
) -> Result<Option<InactiveReason>> {
    if let Some(cond) = &m.os
        && !condition_matches(name, "os", cond, &machine.os, false)?
    {
        return Ok(Some(InactiveReason::Os));
    }
    if let Some(cond) = &m.host
        && !condition_matches(name, "host", cond, &machine.host, true)?
    {
        return Ok(Some(InactiveReason::Host));
    }
    Ok(None)
}

/// Whether any pattern in the condition matches `value`. `glob` enables the
/// edge-`*` forms (host patterns); `os` values are matched exactly.
fn condition_matches(
    name: &str,
    field: &str,
    cond: &MachineMatch,
    value: &str,
    glob: bool,
) -> Result<bool> {
    let patterns: Vec<&str> = match cond {
        MachineMatch::String(s) => vec![s.as_str()],
        MachineMatch::Array(items) => items.iter().map(String::as_str).collect(),
    };
    if patterns.is_empty() {
        return Err(Error::config(format!(
            "mapping `{name}`: `{field}` must not be an empty list"
        )));
    }
    // Validate every pattern before matching any, so a bad pattern is a config
    // error on every machine — not only on the machines where no earlier
    // pattern happens to match.
    for pattern in &patterns {
        if pattern.is_empty() {
            return Err(Error::config(format!(
                "mapping `{name}`: `{field}` contains an empty pattern"
            )));
        }
        if !glob && pattern.contains('*') {
            return Err(Error::config(format!(
                "mapping `{name}`: `{field}` does not support `*` patterns"
            )));
        }
        if glob && pattern.trim_matches('*').contains('*') {
            return Err(Error::config(format!(
                "mapping `{name}`: `{field}` pattern `{pattern}` has `*` in the \
                 middle; `*` may only open or close a pattern"
            )));
        }
    }
    Ok(patterns.iter().any(|p| pattern_matches(p, value, glob)))
}

/// Case-insensitive match of one pattern. With `glob`, `*` at the pattern's
/// edges matches any (possibly empty) run of characters.
fn pattern_matches(pattern: &str, value: &str, glob: bool) -> bool {
    let value = value.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    if !glob {
        return pattern == value;
    }
    let open = pattern.starts_with('*');
    let close = pattern.ends_with('*');
    let core = pattern.trim_matches('*');
    match (open, close) {
        (false, false) => value == core,
        (true, false) => value.ends_with(core),
        (false, true) => value.starts_with(core),
        // `*core*`: substring. `**` has an empty core and matches anything.
        (true, true) => value.contains(core),
    }
}

fn pick_root(
    mapping_value: Option<&str>,
    settings_value: Option<&str>,
    name: &str,
    field: &str,
) -> Result<String> {
    mapping_value
        .or(settings_value)
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::config(format!(
                "mapping `{name}` is missing `{field}` (no mapping or [settings] value)"
            ))
        })
}

/// Expand `~`/env in a path string and make it absolute, using the process's
/// home directory. Convenience over [`expand_path`] for callers (like the CLI)
/// that don't carry a home around.
pub fn expand_root(raw: &str) -> Result<PathBuf> {
    expand_path(raw, home_dir().ok().as_deref())
}

/// Expand `~` and `$VAR`/`${VAR}`, then make the path absolute.
pub fn expand_path(raw: &str, home: Option<&Path>) -> Result<PathBuf> {
    let expanded = expand_env(raw);

    let with_home = if expanded == "~" || expanded.starts_with("~/") {
        let home = home.ok_or(Error::NoHome)?;
        if expanded == "~" {
            home.to_path_buf()
        } else {
            home.join(&expanded[2..])
        }
    } else {
        PathBuf::from(expanded)
    };

    if with_home.is_absolute() {
        Ok(with_home)
    } else {
        std::path::absolute(&with_home).map_err(|e| Error::io(&with_home, e))
    }
}

/// Expand `$VAR` and `${VAR}` using the process environment. Unknown variables
/// expand to empty (matching common shell behaviour).
fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'{' {
                if let Some(end) = s[i + 2..].find('}') {
                    let name = &s[i + 2..i + 2 + end];
                    out.push_str(&std::env::var(name).unwrap_or_default());
                    i = i + 2 + end + 1;
                    continue;
                }
            } else {
                let rest = &s[i + 1..];
                let len = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(rest.len());
                if len > 0 {
                    let name = &rest[..len];
                    out.push_str(&std::env::var(name).unwrap_or_default());
                    i += 1 + len;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(s: &str) -> Config {
        toml::from_str(s).expect("valid toml")
    }

    /// A pinned machine identity so condition tests are host-independent.
    fn tm() -> MachineContext {
        MachineContext {
            os: "linux".into(),
            host: "wrk-01.example".into(),
        }
    }

    /// [`resolve`] under the pinned test machine.
    fn resolve_t(c: Config) -> Result<ResolvedConfig> {
        resolve(c, &tm())
    }

    fn merge_all(docs: &[&str]) -> Config {
        docs.iter()
            .fold(Config::default(), |acc, d| merge(acc, cfg(d)))
    }

    #[test]
    fn hardlink_mode_is_rejected() {
        // hardlink mode was removed; a config using it must fail to load.
        let err = toml::from_str::<Config>("[settings]\nmode = \"hardlink\"\n").unwrap_err();
        assert!(
            err.to_string().contains("hardlink") || err.to_string().contains("unknown variant"),
            "expected a clear rejection, got: {err}"
        );
    }

    #[test]
    fn sync_mode_is_rejected() {
        // `mode = "sync"` was renamed to `"copy"` (PLAN-008, hard break); an
        // old config must fail to load, and the error names the valid variants.
        let err = toml::from_str::<Config>("[settings]\nmode = \"sync\"\n").unwrap_err();
        assert!(
            err.to_string().contains("unknown variant"),
            "expected unknown-variant rejection, got: {err}"
        );
    }

    #[test]
    fn settings_merge_per_key() {
        let merged = merge_all(&[
            r#"[settings]
            live = "~"
            store = "~/dotfiles"
            mode = "symlink""#,
            r#"[settings]
            mode = "copy"
            conflict = "skip""#,
        ]);
        let s = merged.settings.unwrap();
        assert_eq!(s.live.as_deref(), Some("~")); // kept from doc 1
        assert_eq!(s.store.as_deref(), Some("~/dotfiles")); // kept from doc 1
        assert_eq!(s.mode, Some(Mode::Copy)); // overridden by doc 2
        assert_eq!(s.conflict, Some(Conflict::Skip)); // added by doc 2
    }

    #[test]
    fn mappings_accumulate_and_same_name_deep_merge() {
        let merged = merge_all(&[
            r#"[mappings.a.links]
            x = true"#,
            r#"[mappings.a]
            mode = "copy"
            [mappings.a.links]
            y = ""
            [mappings.b.links]
            z = true"#,
        ]);
        let a = &merged.mappings["a"];
        assert_eq!(a.mode, Some(Mode::Copy)); // override applied
        assert!(a.links.contains_key("x")); // combined
        assert!(a.links.contains_key("y"));
        assert!(merged.mappings.contains_key("b")); // distinct name accumulates
    }

    #[test]
    fn links_later_wins_on_duplicate_key() {
        let merged = merge_all(&[
            r#"[mappings.a.links]
            x = true"#,
            r#"[mappings.a.links]
            x = "explicit/path""#,
        ]);
        assert_eq!(
            merged.mappings["a"].links["x"],
            LinkValue::String("explicit/path".into())
        );
    }

    #[test]
    fn resolve_applies_mode_and_conflict_defaults() {
        let c = cfg(r#"[mappings.a]
            live = "/live"
            store = "/store"
            [mappings.a.links]
            ".bashrc" = true"#);
        let r = resolve_t(c).unwrap();
        assert_eq!(r.mappings.len(), 1);
        assert_eq!(r.mappings[0].mode, DEFAULT_MODE);
        assert_eq!(r.mappings[0].conflict, DEFAULT_CONFLICT);
    }

    #[test]
    fn resolve_uses_settings_then_mapping_overrides() {
        let c = cfg(r#"[settings]
            live = "/live"
            store = "/store"
            mode = "copy"
            [mappings.a]
            mode = "symlink"
            [mappings.a.links]
            x = true"#);
        let r = resolve_t(c).unwrap();
        assert_eq!(r.mappings[0].live, PathBuf::from("/live")); // from settings
        assert_eq!(r.mappings[0].mode, Mode::Symlink); // mapping wins over settings
    }

    #[test]
    fn resolve_errors_when_root_missing() {
        let c = cfg(r#"[mappings.a]
            live = "/live"
            [mappings.a.links]
            x = true"#);
        let err = resolve_t(c).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
        assert!(err.to_string().contains("store"));
    }

    #[test]
    fn resolve_rejects_live_equal_store() {
        let c = cfg(r#"[settings]
            live = "/same"
            store = "/same"
            [mappings.a.links]
            x = true"#);
        let err = resolve_t(c).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
        assert!(err.to_string().contains("same directory"));
    }

    #[test]
    fn resolve_sorts_mappings_and_links() {
        let c = cfg(r#"[settings]
            live = "/live"
            store = "/store"
            [mappings.zeta.links]
            b = true
            a = true
            [mappings.alpha.links]
            x = true"#);
        let r = resolve_t(c).unwrap();
        assert_eq!(r.mappings[0].name, "alpha");
        assert_eq!(r.mappings[1].name, "zeta");
        let keys: Vec<&str> = r.mappings[1]
            .links
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["a", "b"]); // links sorted by key
    }

    #[test]
    fn expand_path_handles_tilde_abs_and_relative() {
        let home = Path::new("/home/test");
        assert_eq!(expand_path("~", Some(home)).unwrap(), home);
        assert_eq!(
            expand_path("~/dotfiles", Some(home)).unwrap(),
            home.join("dotfiles")
        );
        assert_eq!(
            expand_path("/absolute/path", Some(home)).unwrap(),
            PathBuf::from("/absolute/path")
        );
        // relative becomes absolute (anchored at cwd)
        assert!(expand_path("rel/dir", Some(home)).unwrap().is_absolute());
    }

    #[test]
    fn expand_env_substitutes_variables() {
        // SAFETY: single-threaded test; nextest runs each test in its own process.
        unsafe {
            std::env::set_var("SYMIFY_TEST_VAR", "VALUE");
        }
        assert_eq!(expand_env("a/$SYMIFY_TEST_VAR/b"), "a/VALUE/b");
        assert_eq!(expand_env("a/${SYMIFY_TEST_VAR}/b"), "a/VALUE/b");
        assert_eq!(expand_env("a/$SYMIFY_UNSET_XYZ/b"), "a//b");
        assert_eq!(expand_env("no vars here"), "no vars here");
    }

    #[test]
    fn starter_template_is_valid_and_carries_roots() {
        let text = render_starter("~", "~/dotfiles");
        let cfg: Config = toml::from_str(&text).expect("starter template parses");
        let s = cfg.settings.unwrap();
        assert_eq!(s.live.as_deref(), Some("~"));
        assert_eq!(s.store.as_deref(), Some("~/dotfiles"));
        // Example links are commented out, so the mapping has no entries.
        assert!(cfg.mappings["dotfiles"].links.is_empty());
    }

    #[test]
    fn discover_cli_configs_replace_defaults() {
        let cli = vec![PathBuf::from("/tmp/custom.toml")];
        assert_eq!(discover(&cli).unwrap(), cli);
    }

    #[test]
    fn discover_in_orders_default_then_conf_d() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        std::fs::write(base.join("symify.toml"), "").unwrap();
        let conf_d = base.join("conf.d");
        std::fs::create_dir(&conf_d).unwrap();
        std::fs::write(conf_d.join("20-b.toml"), "").unwrap();
        std::fs::write(conf_d.join("10-a.toml"), "").unwrap();
        std::fs::write(conf_d.join("ignore.txt"), "").unwrap();

        let found = discover_in(base).unwrap();
        assert_eq!(
            found,
            vec![
                base.join("symify.toml"),
                conf_d.join("10-a.toml"),
                conf_d.join("20-b.toml"),
            ]
        );
    }

    #[test]
    fn select_keeps_named_mappings_and_errors_on_unknown() {
        let cfg = resolve_t(cfg(r#"[settings]
            live = "/l"
            store = "/s"
            [mappings.a.links]
            x = true
            [mappings.b.links]
            y = true
            [mappings.c.links]
            z = true"#))
        .unwrap();

        // empty -> unchanged
        assert_eq!(select(cfg.clone(), &[]).unwrap().mappings.len(), 3);

        // subset, order preserved (resolved order is sorted: a, b, c)
        let sub = select(cfg.clone(), &["c".into(), "a".into()]).unwrap();
        let names: Vec<&str> = sub.mappings.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "c"]);

        // unknown name errors
        assert!(select(cfg, &["nope".into()]).is_err());
    }

    #[test]
    fn ensure_config_auto_inits_then_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: nextest runs each test in its own process.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }
        let expected = dir.path().join("symify").join("symify.toml");

        let first = ensure_config(&[]).unwrap();
        assert_eq!(first.created.as_deref(), Some(expected.as_path()));
        assert!(expected.is_file());
        // the auto-created config is valid and resolves
        resolve_t(load(&first.paths).unwrap()).unwrap();

        // second call finds it; nothing created
        let second = ensure_config(&[]).unwrap();
        assert!(second.created.is_none());
        assert_eq!(second.paths, vec![expected]);
    }

    #[test]
    fn config_base_dir_honours_xdg_then_falls_back_to_home() {
        // SAFETY: nextest runs each test in its own process, so env edits here
        // can't race other tests.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg");
        }
        assert_eq!(config_base_dir().unwrap(), PathBuf::from("/tmp/xdg/symify"));
        assert_eq!(
            default_config_path().unwrap(),
            PathBuf::from("/tmp/xdg/symify/symify.toml")
        );

        // An empty XDG_CONFIG_HOME falls back to ~/.config. $HOME steers
        // home_dir only on Unix; on Windows `directories` resolves the home
        // via the known-folder API, which env vars cannot redirect — so pin
        // the home on Unix and assert only the shape elsewhere.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "");
            #[cfg(unix)]
            std::env::set_var("HOME", "/home/tester");
        }
        #[cfg(unix)]
        {
            assert_eq!(home_dir().unwrap(), PathBuf::from("/home/tester"));
            assert_eq!(
                config_base_dir().unwrap(),
                PathBuf::from("/home/tester/.config/symify")
            );
        }
        #[cfg(not(unix))]
        assert_eq!(
            config_base_dir().unwrap(),
            home_dir().unwrap().join(".config").join("symify")
        );
    }

    #[test]
    fn malformed_toml_reports_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this = = not valid").unwrap();
        let err = load(std::slice::from_ref(&path)).unwrap_err();
        // Current contract: a parse failure is an `Error::Toml` naming the file.
        assert!(matches!(err, Error::Toml { .. }));
        assert!(err.to_string().contains("bad.toml"), "got: {err}");
    }

    // ---- os / host machine conditions ----

    /// Resolve one mapping with the given `os`/`host` condition lines under the
    /// pinned test machine (`linux`, host `wrk-01.example`).
    fn resolve_cond(cond: &str) -> Result<ResolvedConfig> {
        resolve_t(cfg(&format!(
            "[mappings.m]\nlive = \"/l\"\nstore = \"/s\"\n{cond}\n"
        )))
    }

    fn inactive_of(cond: &str) -> Option<InactiveReason> {
        resolve_cond(cond).unwrap().mappings[0].inactive
    }

    #[test]
    fn machine_conditions_match_table() {
        use InactiveReason::{Host, Os};
        for (cond, want) in [
            // Absent conditions: always active.
            ("", None),
            // os: exact, case-insensitive, arrays are alternatives.
            ("os = \"linux\"", None),
            ("os = \"Linux\"", None),
            ("os = \"macos\"", Some(Os)),
            ("os = [\"macos\", \"linux\"]", None),
            ("os = [\"macos\", \"windows\"]", Some(Os)),
            // host: exact and edge globs, case-insensitive.
            ("host = \"wrk-01.example\"", None),
            ("host = \"WRK-01.EXAMPLE\"", None),
            ("host = \"wrk-01\"", Some(Host)),
            ("host = \"wrk-*\"", None),
            ("host = \"*.example\"", None),
            ("host = \"*01.ex*\"", None),
            ("host = [\"laptop\", \"wrk-*\"]", None),
            ("host = [\"laptop\", \"desk-*\"]", Some(Host)),
            // Both keys AND together; report the first mismatch.
            ("os = \"linux\"\nhost = \"wrk-*\"", None),
            ("os = \"macos\"\nhost = \"wrk-*\"", Some(Os)),
            ("os = \"linux\"\nhost = \"desk-*\"", Some(Host)),
        ] {
            assert_eq!(inactive_of(cond), want, "condition: {cond}");
        }
    }

    #[test]
    fn backup_keep_resolves_with_mapping_override() {
        let r = resolve_t(cfg(r#"[settings]
            live = "/l"
            store = "/s"
            backup_keep = 3

            [mappings.a]
            [mappings.b]
            backup_keep = 1"#))
        .unwrap();
        // Sorted by name: a inherits the settings value, b overrides it.
        assert_eq!(r.mappings[0].backup_keep, 3);
        assert_eq!(r.mappings[1].backup_keep, 1);

        // Absent everywhere ⇒ 0 (unlimited).
        let r = resolve_t(cfg(
            "[settings]\nlive = \"/l\"\nstore = \"/s\"\n\n[mappings.m]",
        ))
        .unwrap();
        assert_eq!(r.mappings[0].backup_keep, 0);
    }

    #[test]
    fn machine_condition_config_errors() {
        // Mid-pattern `*`, empty lists, empty patterns, and `*` in `os` are
        // config errors, not silent mismatches — even when an earlier pattern
        // in the list already matches this machine (validity must not depend
        // on which machine reads the config).
        for cond in [
            "host = \"wrk-*-01\"",
            "host = []",
            "host = [\"\"]",
            "os = \"lin*\"",
            "host = [\"wrk-01.example\", \"bad*mid\"]",
            "os = [\"linux\", \"mac*\"]",
        ] {
            assert!(
                resolve_cond(cond).is_err(),
                "expected config error for: {cond}"
            );
        }
    }

    #[test]
    fn conditions_merge_like_other_mapping_keys() {
        // A later file's condition overrides, and conditions survive a merge
        // that only adds links.
        let merged = merge_all(&[
            "[mappings.m]\nos = \"macos\"\n[mappings.m.links]\n\"a\" = true",
            "[mappings.m]\nos = \"linux\"",
        ]);
        let r = resolve(
            Config {
                settings: cfg("[settings]\nlive = \"/l\"\nstore = \"/s\"").settings,
                mappings: merged.mappings,
            },
            &tm(),
        )
        .unwrap();
        assert_eq!(r.mappings[0].inactive, None);
        assert_eq!(r.mappings[0].links.len(), 1);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        // Current contract: the schema sets `additionalProperties: false`, so the
        // generated types deny unknown fields — an extra key is a hard parse error
        // (a typo'd setting fails loudly rather than being silently ignored).
        let err =
            toml::from_str::<Config>("[settings]\nlive=\"~\"\nstore=\"~/d\"\nbogus_key = 42\n")
                .unwrap_err();
        assert!(
            err.to_string().contains("unknown field"),
            "expected unknown-field rejection, got: {err}"
        );
    }

    #[test]
    fn empty_config_file_loads_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.toml");
        std::fs::write(&path, "").unwrap();
        let c = load(std::slice::from_ref(&path)).unwrap();
        assert!(c.settings.is_none());
        assert!(c.mappings.is_empty());
        // No mappings means nothing needs roots, so resolve succeeds with an
        // empty set rather than erroring.
        let r = resolve_t(c).unwrap();
        assert!(r.mappings.is_empty());
    }
}
