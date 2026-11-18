use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const DEFINES: &str = "common/defines/reach.txt";
const COUNTRIES: &str = "common/country_definitions/reach.txt";
const EVENTS: &str = "events/reach.txt";

const VALID: [&str; 3] = [
    "NAI = {\n    MILITARY_REACH_SEA_MAX = 0.85\n}\n",
    "TST = {\n    regnal_numbers = { Anne = 2 }\n}\n",
    r"namespace = reach
reach.1 = {
    type = country_event
    hidden = yes
    trigger = {
        has_military_reach = { target = root min_reach = 0.3 min_shippable_strength = 42 }
        has_military_reach = { target = capital min_reach = { value = 0.3 } }
    }
}
",
];

const INVALID: [&str; 3] = [
    r"NAI = {
    MILITARY_REACH_SEA_MXA = 0.85
    MILITARY_REACH_SEA_MAX = invalid
}
MILITARY_REACH_MARCH_DISTANCE_HALF = 650
",
    r"TST = {
    regnal_numbrs = { Anne = 2 }
    regnal_numbers = {
        Anne = 2.5
        tiger_missing_regnal_name = 4
    }
}
",
    r"namespace = reach
reach.1 = {
    type = country_event
    hidden = yes
    trigger = {
        has_military_reah = { target = root min_reach = 0.3 }
        has_military_reach = { target = ruler min_reach = 0.3 }
        has_military_reach = { target = root min_shippable_strenght = 42 }
        has_military_reach = { min_reach = 0.3 }
        capital = { has_military_reach = { target = root min_reach = 0.3 } }
    }
}
",
];

fn write_fixture(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn validate(game: &Path, mod_dir: &Path, fixture: [&str; 3]) -> Vec<Value> {
    for (path, content) in [DEFINES, COUNTRIES, EVENTS].into_iter().zip(fixture) {
        write_fixture(mod_dir, path, &format!("\u{feff}{content}"));
    }
    let result = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"))
        .arg("--game")
        .arg(game)
        .arg("--json")
        .arg(mod_dir)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    serde_json::from_slice(&result.stdout).unwrap()
}

fn assert_report(reports: &[Value], path: &str, line: u64, key: &str, message: &str) {
    assert!(
        reports.iter().any(|report| {
            let location = &report["locations"][0];
            report["key"] == key
                && report["message"].as_str().is_some_and(|text| text.contains(message))
                && location["from"] == "MOD"
                && location["path"].as_str().is_some_and(|name| name.replace('\\', "/") == path)
                && location["linenr"] == line
        }),
        "missing {key} containing {message:?} at {path}:{line}\n{reports:#?}"
    );
}

#[test]
fn vic3_115_defines_regnal_numbers_and_military_reach() {
    let temp = tempfile::tempdir().unwrap();
    let game = temp.path().join("install");
    let mod_dir = temp.path().join("mod");
    for dir in ["jomini", "clausewitz"] {
        fs::create_dir_all(game.join(dir)).unwrap();
    }
    write_fixture(&game, "launcher/launcher-settings.json", r#"{"rawVersion":"1.15.1"}"#);
    write_fixture(&game, "game/events/titanic_events.txt", "\u{feff}");
    write_fixture(&mod_dir, ".metadata/metadata.json", r#"{"name":"Victoria 3 1.15 fields"}"#);
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
        write_fixture(
            &mod_dir,
            &format!("localization/{lang}/reach_l_{lang}.yml"),
            &format!("\u{feff}l_{lang}:\n TST: \"Test\"\n TST_ADJ: \"Test\"\n Anne: \"Anne\"\n"),
        );
    }

    let clean = validate(&game, &mod_dir, VALID);
    assert!(clean.is_empty(), "{clean:#?}");

    let reports = validate(&game, &mod_dir, INVALID);
    for (path, line, key, message) in [
        (DEFINES, 2, "unknown-field", "NAI|MILITARY_REACH_SEA_MXA"),
        (DEFINES, 3, "validation", "expected number"),
        (DEFINES, 5, "structure", "expected definition"),
        (COUNTRIES, 2, "unknown-field", "regnal_numbrs"),
        (COUNTRIES, 4, "validation", "expected integer"),
        (COUNTRIES, 5, "missing-localization", "tiger_missing_regnal_name"),
        (EVENTS, 6, "unknown-field", "has_military_reah"),
        (EVENTS, 7, "scopes", "expected"),
        (EVENTS, 8, "field-missing", "min_reach"),
        (EVENTS, 8, "unknown-field", "min_shippable_strenght"),
        (EVENTS, 9, "field-missing", "target"),
        (EVENTS, 10, "scopes", "scope seems to be state"),
    ] {
        assert_report(&reports, path, line, key, message);
    }

    let recovered = validate(&game, &mod_dir, VALID);
    assert!(recovered.is_empty(), "{recovered:#?}");
}
