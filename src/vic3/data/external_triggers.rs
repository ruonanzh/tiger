//! Find candidate declarations in the explicitly provided Workshop directory.
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::fileset::LoadedMod;
use crate::helpers::{TigerHashMap, TigerHashSet};

pub(crate) fn index(
    workshop: &Path,
    mod_root: &Path,
    loaded: &[LoadedMod],
) -> TigerHashMap<String, PathBuf> {
    let mut candidates = TigerHashMap::default();
    let Ok(entries) = fs::read_dir(workshop) else { return candidates };
    let active: TigerHashSet<_> = std::iter::once(mod_root)
        .chain(loaded.iter().map(LoadedMod::root))
        .filter_map(|path| path.canonicalize().ok())
        .collect();
    for entry in entries.flatten() {
        let id = entry.file_name();
        if !id.to_str().is_some_and(|id| id.bytes().all(|b| b.is_ascii_digit()))
            || !entry.file_type().is_ok_and(|ty| ty.is_dir())
        {
            continue;
        }
        let root = entry.path();
        let Ok(canonical_root) = root.canonicalize() else { continue };
        if active.contains(&canonical_root) {
            continue;
        }
        let trigger_root = root.join("common/scripted_triggers");
        if !trigger_root.canonicalize().is_ok_and(|path| path.starts_with(&canonical_root)) {
            continue;
        }
        for file in WalkDir::new(trigger_root).into_iter().filter_map(Result::ok) {
            if !file.file_type().is_file() || !file.path().extension().is_some_and(|x| x == "txt") {
                continue;
            }
            let Ok(source) = fs::read_to_string(file.path()) else { continue };
            let Ok(path) = file.path().strip_prefix(workshop) else { continue };
            for name in top_level_declarations(&source) {
                let candidate =
                    candidates.entry(name.to_owned()).or_insert_with(|| path.to_path_buf());
                if path < candidate.as_path() {
                    *candidate = path.to_path_buf();
                }
            }
        }
    }
    candidates
}

/// Recognize only names preceding a top-level block assignment. A candidate never
/// participates in normal loading, macro expansion, validation, or replacement.
fn top_level_declarations(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut chars = source.trim_start_matches('\u{feff}').char_indices().peekable();
    let source = source.trim_start_matches('\u{feff}');
    let mut depth = 0usize;
    let mut name = None;
    let mut assigning = false;
    let mut declaration = None;
    while let Some((start, ch)) = chars.next() {
        match ch {
            '#' => while chars.next_if(|(_, ch)| *ch != '\n').is_some() {},
            '"' => {
                let mut end = None;
                while let Some((i, ch)) = chars.next() {
                    if ch == '\\' {
                        chars.next();
                    } else if ch == '"' {
                        end = Some(i);
                        break;
                    }
                }
                if depth == 0 {
                    name = end.map(|i| &source[start + 1..i]).filter(|s| s.chars().all(name_char));
                    assigning = false;
                }
            }
            '{' => {
                if depth == 0 && assigning {
                    declaration = name
                        .take()
                        .and_then(|key| match key.split_once(':') {
                            Some(("REPLACE_OR_CREATE" | "INJECT_OR_CREATE", key)) => Some(key),
                            None => Some(key),
                            _ => None,
                        })
                        .filter(|key| !key.is_empty() && !key.contains(':'));
                }
                depth += 1;
                assigning = false;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(key) = declaration.take()
                {
                    out.push(key);
                }
                name = None;
                assigning = false;
            }
            '=' if depth == 0 && !assigning => {
                assigning = name.is_some();
            }
            ch if depth == 0 && !ch.is_whitespace() => {
                while chars
                    .next_if(|(_, ch)| {
                        !ch.is_whitespace() && !matches!(ch, '#' | '"' | '=' | '{' | '}')
                    })
                    .is_some()
                {}
                let end = chars.peek().map_or(source.len(), |(i, _)| *i);
                let token = &source[start..end];
                name = token.chars().all(name_char).then_some(token);
                assigning = false;
            }
            _ => {}
        }
    }
    out
}

fn name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':')
}

#[test]
fn recognizes_declarations_without_indexing_comment_or_body_keys() {
    let source = r#"
# hidden = {}
"external_one"
  = { nested = { } text = "quoted } {" }
REPLACE_OR_CREATE:external_two = {}
TRY_REPLACE:existing_only = {}
external_three = {}
malformed == {}
@reader_variable = {}
broken_macro = {
"#;
    assert_eq!(top_level_declarations(source), ["external_one", "external_two", "external_three"]);
}
