//! Round-trip edits of a Starship config. `toml_edit` keeps comments,
//! key order, and every key this screen does not touch.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use toml_edit::{DocumentMut, Item, Table};

/// `$all` in the order `starship print-config` documents for this install.
pub const ALL_MODULES: &[&str] = &[
    "username",
    "hostname",
    "localip",
    "shlvl",
    "singularity",
    "kubernetes",
    "nats",
    "directory",
    "vcsh",
    "fossil_branch",
    "fossil_metrics",
    "git_branch",
    "git_commit",
    "git_state",
    "git_metrics",
    "git_status",
    "hg_branch",
    "hg_state",
    "pijul_channel",
    "docker_context",
    "package",
    "bun",
    "c",
    "cmake",
    "cobol",
    "cpp",
    "daml",
    "dart",
    "deno",
    "dotnet",
    "elixir",
    "elm",
    "erlang",
    "fennel",
    "fortran",
    "gleam",
    "golang",
    "gradle",
    "haskell",
    "haxe",
    "helm",
    "java",
    "julia",
    "kotlin",
    "lua",
    "maven",
    "mojo",
    "nim",
    "nodejs",
    "ocaml",
    "odin",
    "opa",
    "perl",
    "php",
    "pulumi",
    "purescript",
    "python",
    "quarto",
    "raku",
    "rlang",
    "red",
    "ruby",
    "rust",
    "scala",
    "solidity",
    "swift",
    "terraform",
    "typst",
    "vlang",
    "vagrant",
    "xmake",
    "zig",
    "buf",
    "guix_shell",
    "nix_shell",
    "conda",
    "pixi",
    "meson",
    "spack",
    "memory_usage",
    "aws",
    "gcloud",
    "openstack",
    "azure",
    "direnv",
    "env_var",
    "mise",
    "crystal",
    "custom",
    "sudo",
    "cmd_duration",
    "line_break",
    "jobs",
    "battery",
    "time",
    "status",
    "container",
    "netns",
    "os",
    "shell",
    "character",
];

pub const PROMPT_FIELDS: &[&str] = &[
    "format",
    "right_format",
    "add_newline",
    "scan_timeout",
    "command_timeout",
];

pub const MODULE_FIELDS: &[&str] = &["symbol", "format", "style", "disabled"];

#[derive(Clone, Debug)]
pub struct ConfigDoc {
    pub doc: DocumentMut,
}

impl ConfigDoc {
    pub fn parse(text: &str) -> Self {
        let doc = text
            .parse::<DocumentMut>()
            .expect("ConfigDoc::parse requires already validated TOML");
        Self { doc }
    }

    pub fn to_toml(&self) -> String {
        self.doc.to_string()
    }
}

pub fn validate(doc: &DocumentMut) -> Result<(), String> {
    fn check(table: &dyn toml_edit::TableLike, prefix: &str) -> Result<(), String> {
        if prefix.starts_with("palettes.")
            || prefix == "palettes"
            || prefix == "profiles"
            || prefix.starts_with("profiles.")
        {
            return Ok(());
        }
        let owner = prefix.split('.').next().unwrap_or("");
        if !prefix.is_empty()
            && !ALL_MODULES.contains(&owner)
            && owner != "env_var"
            && owner != "custom"
        {
            return Ok(());
        }
        for (key, item) in table.iter() {
            let path = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{prefix}.{key}")
            };
            if let Some(child) = item.as_table_like() {
                check(child, &path)?;
                continue;
            }
            if prefix.is_empty() && !PROMPT_FIELDS.contains(&key) && key != "follow_symlinks" {
                continue;
            }
            let expected = match key {
                "format" | "right_format" | "style" | "symbol" | "command" => {
                    Some((item.as_str().is_some(), "a string"))
                }
                "disabled" | "add_newline" | "show_always" | "ssh_only" | "follow_symlinks" => {
                    Some((item.as_bool().is_some(), "true or false"))
                }
                "scan_timeout" | "command_timeout" | "min_time" | "truncation_length" => Some((
                    item.as_integer().is_some_and(|n| n >= 0),
                    "a nonnegative integer",
                )),
                "detect_extensions" | "detect_files" | "detect_folders" => Some((
                    item.as_array()
                        .is_some_and(|a| a.iter().all(|v| v.as_str().is_some())),
                    "an array of strings",
                )),
                _ => None,
            };
            if let Some((valid, expected)) = expected {
                if !valid {
                    return Err(format!("{path} requires {expected}"));
                }
            }
        }
        Ok(())
    }
    check(doc.as_table(), "")
}

pub fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("STARSHIP_CONFIG") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/starship.toml")
}

pub fn read_config(path: &Path) -> anyhow::Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

/// Write beside the resolved target, preserve mode, sync, then atomically rename.
pub fn write_config(path: &Path, text: &str, backup: bool) -> anyhow::Result<Option<PathBuf>> {
    use std::io::Write;
    text.parse::<DocumentMut>()
        .context("invalid TOML; nothing saved")?;
    let target = if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        fs::canonicalize(path).context("could not resolve configuration symlink")?
    } else {
        path.to_path_buf()
    };
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;
    let metadata = fs::metadata(&target).ok();
    anyhow::ensure!(
        !metadata
            .as_ref()
            .is_some_and(|m| m.permissions().readonly()),
        "Target is read-only; choose Save as or fix permissions"
    );
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    if let Some(meta) = &metadata {
        file.as_file().set_permissions(meta.permissions())?;
    }
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    let copied = if backup && metadata.is_some() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let name = target.file_name().unwrap_or_default().to_string_lossy();
        let dest = target.with_file_name(format!("{name}.bak.{stamp}"));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)?;
        std::io::copy(&mut fs::File::open(&target)?, &mut output)?;
        if let Some(meta) = &metadata {
            output.set_permissions(meta.permissions())?;
        }
        output.sync_all()?;
        Some(dest)
    } else {
        None
    };
    file.persist(&target)
        .map_err(|e| e.error)
        .with_context(|| format!("could not save {}", target.display()))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(copied)
}

pub fn format_of(doc: &DocumentMut) -> Option<String> {
    doc.get("format").and_then(Item::as_str).map(str::to_string)
}

pub fn logical_format(doc: &DocumentMut) -> String {
    match format_of(doc) {
        Some(text) => text,
        _ => "$all".to_string(),
    }
}

pub fn set_format(doc: &mut DocumentMut, format: &str) {
    doc["format"] = toml_edit::value(format);
}

/// Byte ranges of `$name` and `${name}`. `$c` does not match `$cmd_duration`.
pub fn find_vars(format: &str, name: &str) -> Vec<(usize, usize)> {
    let bytes = format.as_bytes();
    let needle = name.as_bytes();
    let mut hits = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$'
            || (i > 0 && bytes[..i].iter().rev().take_while(|b| **b == b'\\').count() % 2 == 1)
        {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if i < bytes.len() && bytes[i] == b'{' {
            i += 1;
            let name_start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            let name_end = i;
            if i < bytes.len() {
                i += 1;
            }
            if &bytes[name_start..name_end] == needle {
                hits.push((start, i));
            }
            continue;
        }
        let name_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
        {
            i += 1;
        }
        if &bytes[name_start..i] == needle {
            hits.push((start, i));
        }
    }
    hits
}

#[cfg(test)]
pub fn remove_var(format: &str, name: &str) -> String {
    let hits = find_vars(format, name);
    if hits.is_empty() {
        return format.to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    for (start, end) in hits {
        out.push_str(&format[last..start]);
        last = end;
    }
    out.push_str(&format[last..]);
    out
}

/// Insert `$name` before `$line_break` or `$character` when those exist.

#[cfg(test)]
pub fn field_value(doc: &DocumentMut, module: Option<&str>, field: &str) -> String {
    let item = match module {
        Some(module) => doc
            .get(module)
            .and_then(Item::as_table)
            .and_then(|table| table.get(field)),
        None => doc.get(field),
    };
    let Some(item) = item else {
        return String::new();
    };
    match field {
        "disabled" | "add_newline" => item.as_bool().map(|v| v.to_string()).unwrap_or_default(),
        "scan_timeout" | "command_timeout" => item
            .as_integer()
            .map(|v| v.to_string())
            .unwrap_or_else(|| item.as_str().unwrap_or("").to_string()),
        _ => item.as_str().unwrap_or("").to_string(),
    }
}

#[cfg(test)]
pub fn set_field(
    doc: &mut DocumentMut,
    module: Option<&str>,
    field: &str,
    raw: &str,
) -> Result<(), String> {
    let item = match field {
        "disabled" | "add_newline" => {
            let value = parse_bool(raw)?;
            toml_edit::value(value)
        }
        "scan_timeout" | "command_timeout" => {
            let value = raw
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("{field} needs a number"))?;
            toml_edit::value(value)
        }
        _ => toml_edit::value(raw),
    };
    match module {
        Some(module) => {
            module_table(doc, module).insert(field, item);
        }
        None => {
            doc[field] = item;
        }
    }
    Ok(())
}

pub fn clear_symbols(doc: &mut DocumentMut) {
    let keys: Vec<String> = doc
        .iter()
        .filter_map(|(key, item)| item.as_table().map(|_| key.to_string()))
        .collect();
    for key in keys {
        let Some(table) = doc.get(&key).and_then(Item::as_table) else {
            continue;
        };
        let fields: Vec<String> = table
            .iter()
            .filter(|(field, value)| is_symbol_field(field) && !value.is_table())
            .map(|(field, _)| field.to_string())
            .collect();
        let has_os = key == "os" && table.get("symbols").and_then(Item::as_table).is_some();
        let os_names: Vec<String> = if has_os {
            table
                .get("symbols")
                .and_then(Item::as_table)
                .map(|symbols| symbols.iter().map(|(name, _)| name.to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if !fields.is_empty() {
            let table = module_table(doc, &key);
            for field in fields {
                table.insert(&field, toml_edit::value(""));
            }
        }
        if has_os {
            let symbols = os_symbols_mut(doc);
            for name in os_names {
                symbols.insert(&name, toml_edit::value(""));
            }
        }
    }
}

fn is_symbol_field(field: &str) -> bool {
    field == "symbol" || field == "read_only" || field.ends_with("_symbol")
}

#[cfg(test)]
fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "true" | "yes" | "on" => Ok(true),
        "false" | "no" | "off" => Ok(false),
        _ => Err("use true or false".to_string()),
    }
}

#[cfg(test)]
pub fn remove_field(doc: &mut DocumentMut, module: Option<&str>, field: &str) {
    match module {
        Some(module) => {
            if let Some(table) = doc.get_mut(module).and_then(Item::as_table_mut) {
                table.remove(field);
            }
        }
        None => {
            doc.remove(field);
        }
    }
}

fn module_table<'a>(doc: &'a mut DocumentMut, module: &str) -> &'a mut Table {
    let item = doc.entry(module).or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("module table")
}

fn os_symbols_mut(doc: &mut DocumentMut) -> &mut Table {
    let os = module_table(doc, "os");
    let item = os.entry("symbols").or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("os.symbols")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_boundaries_do_not_eat_longer_names() {
        let format = "$directory$c$cmd_duration$git_status$git_state$character";
        let removed = remove_var(format, "c");
        assert_eq!(
            removed,
            "$directory$cmd_duration$git_status$git_state$character"
        );
        assert!(find_vars(&removed, "git_state").len() == 1);
        assert!(find_vars(&remove_var(&removed, "git_status"), "git_state").len() == 1);
    }

    #[test]
    fn editor_save_keeps_comments_and_untouched_keys() {
        let original = "\
# keep me
\"$schema\" = \"https://starship.rs/config-schema.json\"

[rust]
symbol = \"R \"

[custom]
other = \"stay\"
";
        let mut doc = ConfigDoc::parse(original);
        set_field(&mut doc.doc, Some("rust"), "symbol", "X ").unwrap();
        set_field(&mut doc.doc, Some("rust"), "disabled", "true").unwrap();
        let saved = doc.to_toml();
        assert!(saved.contains("# keep me"), "{saved}");
        assert!(saved.contains("other = \"stay\""), "{saved}");
        assert!(saved.contains("symbol = \"X \""), "{saved}");
        assert!(saved.contains("disabled = true"), "{saved}");
        assert!(saved.contains("$schema"), "{saved}");
    }

    #[test]
    fn clearing_a_field_removes_the_key() {
        let mut doc = ConfigDoc::parse("[rust]\nsymbol = \"R \"\nstyle = \"red\"\n");
        remove_field(&mut doc.doc, Some("rust"), "symbol");
        let saved = doc.to_toml();
        assert!(!saved.contains("symbol"), "{saved}");
        assert!(saved.contains("style = \"red\""), "{saved}");
    }
}
