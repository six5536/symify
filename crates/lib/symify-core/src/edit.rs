//! Format-preserving config edits for `add`/`remove`, via `toml_edit`.
//!
//! These operate over the discovered config *set* (auto-locate), not a single
//! file: `remove_link` clears a key from every file that defines it, and
//! `add_link` writes beside an existing mapping (highest-precedence file if it's
//! split) or creates a new mapping in the primary file. Comments, ordering, and
//! the `#:schema` line are preserved.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, TableLike, Value};

use crate::error::{Error, Result};
use crate::model::LinkValue;

/// What an [`add_link`] call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddStatus {
    /// A new entry was written.
    Added,
    /// An existing entry's value was overwritten (`force`).
    Replaced,
    /// The entry already had this value; nothing was written.
    Unchanged,
}

/// Result of [`add_link`].
#[derive(Debug, Clone)]
pub struct AddReport {
    /// The file that was edited.
    pub file: PathBuf,
    /// Whether the mapping had to be created in that file.
    pub created_mapping: bool,
    /// What happened to the entry.
    pub status: AddStatus,
}

/// Add (or update) `mapping.links.key = value` in the appropriate file.
///
/// Target file: the highest-precedence file in `files` that already defines the
/// mapping, otherwise `primary`. Errors if the key exists with a different value
/// unless `force`.
pub fn add_link(
    files: &[PathBuf],
    primary: &Path,
    mapping: &str,
    key: &str,
    value: LinkValue,
    force: bool,
) -> Result<AddReport> {
    let target = files
        .iter()
        .rev()
        .find(|f| file_defines_mapping(f, mapping).unwrap_or(false))
        .cloned()
        .unwrap_or_else(|| primary.to_path_buf());

    let mut doc = parse(&target)?;
    let created_mapping = !mapping_present(&doc, mapping);

    let status = match links_table(&doc, mapping)
        .and_then(|t| t.get(key))
        .and_then(item_to_link)
    {
        Some(ref existing) if *existing == value => AddStatus::Unchanged,
        Some(_) if !force => {
            return Err(Error::config(format!(
                "`{key}` already exists in mapping `{mapping}` with a different value; use --force to overwrite"
            )));
        }
        Some(_) => AddStatus::Replaced,
        None => AddStatus::Added,
    };

    if status != AddStatus::Unchanged {
        doc["mappings"][mapping]["links"][key] = link_to_item(&value);
        write(&target, &doc)?;
    }

    Ok(AddReport {
        file: target,
        created_mapping,
        status,
    })
}

/// Remove `mapping.links.key` from every file in `files` that defines it.
/// Returns the edited files; errors if no file contained the key.
pub fn remove_link(files: &[PathBuf], mapping: &str, key: &str) -> Result<Vec<PathBuf>> {
    let mut edited = Vec::new();
    for file in files {
        let mut doc = parse(file)?;
        let present = links_table(&doc, mapping).is_some_and(|t| t.contains_key(key));
        if !present {
            continue;
        }
        if let Some(links) = links_table_mut(&mut doc, mapping) {
            links.remove(key);
        }
        write(file, &doc)?;
        edited.push(file.clone());
    }
    if edited.is_empty() {
        return Err(Error::config(format!(
            "no entry `{key}` in mapping `{mapping}`"
        )));
    }
    Ok(edited)
}

// ----- helpers -----------------------------------------------------------

fn parse(file: &Path) -> Result<DocumentMut> {
    let text = std::fs::read_to_string(file).map_err(|e| Error::io(file, e))?;
    text.parse::<DocumentMut>()
        .map_err(|e| Error::config(format!("failed to parse {}: {e}", file.display())))
}

fn write(file: &Path, doc: &DocumentMut) -> Result<()> {
    std::fs::write(file, doc.to_string()).map_err(|e| Error::io(file, e))
}

fn file_defines_mapping(file: &Path, mapping: &str) -> Result<bool> {
    Ok(mapping_present(&parse(file)?, mapping))
}

fn mapping_present(doc: &DocumentMut, mapping: &str) -> bool {
    doc.get("mappings")
        .and_then(Item::as_table_like)
        .is_some_and(|t| t.contains_key(mapping))
}

fn links_table<'a>(doc: &'a DocumentMut, mapping: &str) -> Option<&'a dyn TableLike> {
    doc.get("mappings")?
        .as_table_like()?
        .get(mapping)?
        .as_table_like()?
        .get("links")?
        .as_table_like()
}

fn links_table_mut<'a>(doc: &'a mut DocumentMut, mapping: &str) -> Option<&'a mut dyn TableLike> {
    doc.get_mut("mappings")?
        .as_table_like_mut()?
        .get_mut(mapping)?
        .as_table_like_mut()?
        .get_mut("links")?
        .as_table_like_mut()
}

fn item_to_link(item: &Item) -> Option<LinkValue> {
    match item.as_value()? {
        Value::Boolean(b) => Some(LinkValue::Boolean(*b.value())),
        Value::String(s) => Some(LinkValue::String(s.value().clone())),
        _ => None,
    }
}

fn link_to_item(value: &LinkValue) -> Item {
    match value {
        LinkValue::Boolean(b) => toml_edit::value(*b),
        LinkValue::String(s) => toml_edit::value(s.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn add_preserves_comments_and_schema_line() {
        let dir = tempfile::tempdir().unwrap();
        let body = "#:schema https://example/symify.schema.json\n\n# my config\n[settings]\nlive = \"~\"\nstore = \"~/dotfiles\"\n\n[mappings.dotfiles.links]\n# example\n";
        let f = write_file(dir.path(), "symify.toml", body);

        let report = add_link(
            std::slice::from_ref(&f),
            &f,
            "dotfiles",
            ".zshrc",
            LinkValue::Boolean(true),
            false,
        )
        .unwrap();
        assert_eq!(report.status, AddStatus::Added);
        assert!(!report.created_mapping);

        let out = std::fs::read_to_string(&f).unwrap();
        assert!(out.contains("#:schema https://example/symify.schema.json"));
        assert!(out.contains("# my config"));
        assert!(out.contains("# example"));
        assert!(out.contains(".zshrc"));
        // still valid and round-trips through the typed model
        let cfg: crate::model::Config = toml::from_str(&out).unwrap();
        assert!(cfg.mappings["dotfiles"].links.contains_key(".zshrc"));
    }

    #[test]
    fn add_creates_missing_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let f = write_file(
            dir.path(),
            "symify.toml",
            "[settings]\nlive = \"~\"\nstore = \"~/s\"\n",
        );
        let report = add_link(
            std::slice::from_ref(&f),
            &f,
            "work",
            "x",
            LinkValue::Boolean(true),
            false,
        )
        .unwrap();
        assert!(report.created_mapping);
        let out = std::fs::read_to_string(&f).unwrap();
        let cfg: crate::model::Config = toml::from_str(&out).unwrap();
        assert!(cfg.mappings["work"].links.contains_key("x"));
    }

    #[test]
    fn add_same_value_is_unchanged_and_differing_needs_force() {
        let dir = tempfile::tempdir().unwrap();
        let f = write_file(dir.path(), "c.toml", "[mappings.m.links]\nx = true\n");
        let files = std::slice::from_ref(&f);

        assert_eq!(
            add_link(files, &f, "m", "x", LinkValue::Boolean(true), false)
                .unwrap()
                .status,
            AddStatus::Unchanged
        );
        // differing value without force errors
        assert!(add_link(files, &f, "m", "x", LinkValue::String("p".into()), false).is_err());
        // with force it replaces
        assert_eq!(
            add_link(files, &f, "m", "x", LinkValue::String("p".into()), true)
                .unwrap()
                .status,
            AddStatus::Replaced
        );
    }

    #[test]
    fn remove_clears_key_from_all_defining_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(
            dir.path(),
            "a.toml",
            "[mappings.m.links]\nx = true\ny = true\n",
        );
        let b = write_file(dir.path(), "b.toml", "[mappings.m.links]\nx = \"p\"\n");
        let files = vec![a.clone(), b.clone()];

        let edited = remove_link(&files, "m", "x").unwrap();
        assert_eq!(edited.len(), 2); // x was in both
        assert!(!std::fs::read_to_string(&a).unwrap().contains("x ="));
        assert!(std::fs::read_to_string(&a).unwrap().contains("y =")); // sibling kept
        assert!(!std::fs::read_to_string(&b).unwrap().contains("x ="));

        // removing a missing key errors
        assert!(remove_link(&files, "m", "nope").is_err());
    }
}
