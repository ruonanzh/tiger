use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const SUBJECT_TYPES: &str = "common/subject_types/versioned.txt";
const ON_ACTIONS: &str = "common/on_actions/versioned.txt";
const RELEASED: &str = "on_diplo_play_subject_released";
const OVERLORD: &str = "on_diplo_play_subject_released_overlord";
const REMOVED_FIELD: &str = "breaks_if_subject_not_protected";

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn validate(game: &Path, mod_dir: &Path, version: Option<&str>) -> (Vec<Value>, String) {
    let launcher = "launcher/launcher-settings.json";
    if let Some(version) = version {
        write(game, launcher, &serde_json::json!({ "rawVersion": version }).to_string());
    } else {
        fs::remove_file(game.join(launcher)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"))
        .arg("--game")
        .arg(game)
        .arg("--json")
        .arg(mod_dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    (serde_json::from_slice(&output.stdout).unwrap(), stderr)
}

fn location_matches(location: &Value, path: &str, line: u64) -> bool {
    location["from"] == "MOD"
        && location["path"].as_str().is_some_and(|name| name.replace('\\', "/") == path)
        && location["linenr"] == line
}

fn assert_report(reports: &[Value], key: &str, message: &str, path: &str, line: u64) {
    assert!(
        reports.iter().any(|report| {
            report["key"] == key
                && report["message"].as_str().is_some_and(|text| text.contains(message))
                && location_matches(&report["locations"][0], path, line)
        }),
        "missing {key} containing {message:?} at {path}:{line}\n{reports:#?}"
    );
}

#[test]
fn subject_release_removals_follow_installed_version() {
    let temp = tempfile::tempdir().unwrap();
    let game = temp.path().join("install");
    let mod_dir = temp.path().join("mod");
    for dir in ["jomini", "clausewitz"] {
        fs::create_dir_all(game.join(dir)).unwrap();
    }
    write(&game, "game/events/titanic_events.txt", "\u{feff}");
    write(&mod_dir, ".metadata/metadata.json", r#"{"name":"Victoria 3 subject release"}"#);
    for lang in [
        "braz_por",
        "english",
        "french",
        "german",
        "japanese",
        "korean",
        "polish",
        "russian",
        "simp_chinese",
        "spanish",
        "turkish",
    ] {
        write(
            &mod_dir,
            &format!("localization/{lang}/versioned_l_{lang}.yml"),
            &format!(
                "\u{feff}l_{lang}:\n tiger_subject: \"Subject\"\n tiger_invalid_subject: \"Invalid subject\"\n"
            ),
        );
    }
    write(
        &mod_dir,
        SUBJECT_TYPES,
        &format!(
            "\u{feff}tiger_subject = {{\n {REMOVED_FIELD} = yes\n}}\ntiger_invalid_subject = {{\n {REMOVED_FIELD} = invalid\n}}\n"
        ),
    );
    write(&mod_dir, ON_ACTIONS, &format!("\u{feff}{RELEASED} = {{}}\n{OVERLORD} = {{}}\n"));

    let (legacy, _) = validate(&game, &mod_dir, Some("1.15.1"));
    assert_eq!(legacy.len(), 1, "{legacy:#?}");
    assert_report(&legacy, "validation", "expected yes or no", SUBJECT_TYPES, 5);

    let (updated, stderr) = validate(&game, &mod_dir, Some("1.15.2"));
    assert!(stderr.contains("Tiger Victoria 3 baseline: 1.13.5 (Matcha); includes selected rule updates through 1.15.2."), "{stderr}");
    assert_eq!(updated.len(), 3, "{updated:#?}");
    assert_report(&updated, "removed", REMOVED_FIELD, SUBJECT_TYPES, 2);
    assert_report(&updated, "removed", REMOVED_FIELD, SUBJECT_TYPES, 5);
    let review = updated.iter().find(|report| report["confidence"] == "weak").unwrap();
    assert_eq!(review["key"], "removed");
    let locations = review["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 2, "{review:#?}");
    assert!(location_matches(&locations[0], ON_ACTIONS, 1));
    assert_eq!(locations[0]["tag"], RELEASED);
    assert!(location_matches(&locations[1], ON_ACTIONS, 2));
    assert_eq!(locations[1]["tag"], OVERLORD);

    for unknown in [Some("1.15"), Some("1.15.2.invalid"), None] {
        let (reports, _) = validate(&game, &mod_dir, unknown);
        assert_eq!(reports, legacy);
    }

    write(
        &mod_dir,
        ON_ACTIONS,
        &format!(
            "\u{feff}{RELEASED} = {{ trigger = {{ is_ai = yes }} }}\non_monthly_pulse_country = {{\n on_actions = {{ {RELEASED} }}\n random_on_action = {{ 20 = {OVERLORD} }}\n}}\n"
        ),
    );
    let (legacy_explicit, _) = validate(&game, &mod_dir, Some("1.15.1"));
    assert_report(&legacy_explicit, "scopes", "diplomatic play", ON_ACTIONS, 1);
    assert_report(&legacy_explicit, "missing-item", OVERLORD, ON_ACTIONS, 4);

    let (explicit, _) = validate(&game, &mod_dir, Some("1.15.2"));
    assert_eq!(explicit.len(), 3, "{explicit:#?}");
    assert_report(&explicit, "missing-item", OVERLORD, ON_ACTIONS, 4);
    let missing = explicit.iter().find(|report| report["key"] == "missing-item").unwrap();
    assert!(missing["info"].as_str().unwrap().contains("1.15.2 removed this engine on_action"));
    assert!(explicit.iter().all(|report| report["confidence"] != "weak"), "{explicit:#?}");

    let private_dispatches = format!(
        "\u{feff}{RELEASED} = {{ on_actions = {{ {RELEASED} {OVERLORD} }} }}\n{OVERLORD} = {{ fallback = tiger_private_link }}\ntiger_private_link = {{ first_valid_on_action = {{ {RELEASED} }} }}\n"
    );
    let engine_entry =
        format!("on_monthly_pulse_country = {{ on_actions = {{ tiger_private_link }} }}\n");
    for entry_first in [None, Some(true), Some(false), None] {
        let fixture = match entry_first {
            None => private_dispatches.clone(),
            Some(true) => format!(
                "\u{feff}{engine_entry}{}",
                private_dispatches.trim_start_matches('\u{feff}')
            ),
            Some(false) => format!("{private_dispatches}{engine_entry}"),
        };
        let has_entry = entry_first.is_some();
        write(&mod_dir, ON_ACTIONS, &fixture);
        let (reports, _) = validate(&game, &mod_dir, Some("1.15.2"));
        assert_eq!(reports.len(), if has_entry { 2 } else { 3 }, "{reports:#?}");
        if !has_entry {
            let review = reports.iter().find(|report| report["confidence"] == "weak").unwrap();
            let locs = review["locations"].as_array().unwrap();
            assert_eq!(locs.len(), 2, "{review:#?}");
            assert_eq!(locs[0]["tag"], RELEASED);
            assert_eq!(locs[1]["tag"], OVERLORD);
        }
    }

    for definitions in [
        format!("{RELEASED} = {{ on_actions = {{ {RELEASED} }} }}\n{OVERLORD} = {{}}\n"),
        format!(
            "{RELEASED} = {{ on_actions = {{ {OVERLORD} }} }}\n{OVERLORD} = {{ on_actions = {{ {RELEASED} }} }}\n"
        ),
    ] {
        write(&mod_dir, ON_ACTIONS, &format!("\u{feff}{definitions}"));
        let (reports, _) = validate(&game, &mod_dir, Some("1.15.2"));
        let review = reports.iter().find(|report| report["confidence"] == "weak").unwrap();
        assert_eq!(review["locations"].as_array().unwrap().len(), 2, "{review:#?}");
    }

    write(
        &mod_dir,
        "events/versioned.txt",
        &format!(
            "\u{feff}namespace = tiger_versioned\ntiger_versioned.1 = {{\n type = country_event\n hidden = yes\n immediate = {{ trigger_event = {{ on_action = {OVERLORD} }} }}\n}}\n"
        ),
    );
    let (explicit_event, _) = validate(&game, &mod_dir, Some("1.15.2"));
    assert_eq!(explicit_event.len(), 2, "{explicit_event:#?}");
    assert!(
        explicit_event
            .iter()
            .all(|report| report["key"] == "removed" && report["confidence"] != "weak")
    );
}
