use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const ENTRY_PATH: &str = "common/journal_entries/test_outcomes.txt";

const JOURNAL_ENTRY: &str = r"je_tiger_test = {
    activation_description = { always = yes }
    timeout = 30
    outcome = {
        header = je_tiger_test_header
        effect_header = je_tiger_test_effect_header
        conclusion_desc = je_tiger_test_conclusion
        trigger = { always = yes }
        effect = { set_variable = tiger_completed }
    }
    outcome = {
        flags = { hidden timeout }
        header = je_tiger_test_header
        effect_header = je_tiger_test_effect_header
        conclusion_desc = je_tiger_test_conclusion
        effect = { set_variable = tiger_timed_out }
    }
}
";

const LOCALIZATION: &str = r#" je_tiger_test: "Test entry"
 je_tiger_test_reason: "Test reason"
 je_tiger_test_header: "Test header"
 je_tiger_test_effect_header: "Test effect header"
 je_tiger_test_conclusion: "Test conclusion"
"#;

#[derive(Debug, Deserialize)]
struct Report {
    key: String,
    message: String,
    locations: Vec<Location>,
}

#[derive(Debug, Deserialize)]
struct Location {
    from: String,
    path: String,
    linenr: usize,
}

fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn validate(game: &Path, mod_dir: &Path, script: &str) -> Vec<Report> {
    write_file(mod_dir, ENTRY_PATH, &format!("\u{feff}{script}"));
    let output = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"))
        .arg("--game")
        .arg(game)
        .arg("--json")
        .arg(mod_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn expect_report(reports: &[Report], key: &str, message: &str, line: usize) {
    assert!(
        reports.iter().any(|report| {
            report.key == key
                && report.message.contains(message)
                && report.locations.iter().any(|location| {
                    location.from == "MOD"
                        && location.path.replace('\\', "/") == ENTRY_PATH
                        && location.linenr == line
                })
        }),
        "missing {key}: {message} at {ENTRY_PATH}:{line}\n{reports:#?}"
    );
}

fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let fixture = tempfile::tempdir().unwrap();
    let game = fixture.path().join("game-installation");
    let mod_dir = fixture.path().join("test-mod");
    for dir in ["clausewitz", "jomini"] {
        fs::create_dir_all(game.join(dir)).unwrap();
    }
    write_file(&game, "game/events/titanic_events.txt", "\u{feff}");
    write_file(&game, "launcher/launcher-settings.json", r#"{"rawVersion":"1.15.1"}"#);
    write_file(
        &mod_dir,
        ".metadata/metadata.json",
        r#"{
            "name": "Journal Entry Test",
            "id": "0",
            "version": "1.0",
            "supported_game_version": "1.15.1",
            "short_description": "Journal entry validation fixture",
            "tags": [],
            "relationships": [],
            "game_custom_data": { "replace_paths": [] }
        }"#,
    );

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
        write_file(
            &mod_dir,
            &format!("localization/{lang}/outcomes_l_{lang}.yml"),
            &format!("\u{feff}l_{lang}:\n{LOCALIZATION}"),
        );
    }

    (fixture, game, mod_dir)
}

#[test]
fn journal_entry_outcomes() {
    let (_fixture, game, mod_dir) = setup();
    let clean = validate(&game, &mod_dir, JOURNAL_ENTRY);
    assert!(clean.is_empty(), "{clean:#?}");

    let hidden = JOURNAL_ENTRY.replace(
        "hidden timeout }\n        header = je_tiger_test_header\n        effect_header = je_tiger_test_effect_header",
        "hidden timeout }",
    );
    assert_ne!(hidden, JOURNAL_ENTRY);
    assert!(validate(&game, &mod_dir, &hidden).is_empty());

    let malformed = JOURNAL_ENTRY
        .replace(
            "activation_description = { always = yes }",
            "activation_description = { set_variable = tiger_activated }",
        )
        .replace("trigger = { always = yes }", "trigger = { set_variable = tiger_completed }")
        .replace("effect = { set_variable = tiger_completed }", "effect = { is_subject = yes }")
        .replace("hidden timeout", "hidden timeout imaginary_flag")
        .replace("effect = { set_variable = tiger_timed_out }", "unexpected_field = yes");
    let reports = validate(&game, &mod_dir, &malformed);
    expect_report(&reports, "wrong-use", "`set_variable` is an effect", 2);
    expect_report(&reports, "wrong-use", "`set_variable` is an effect", 8);
    expect_report(&reports, "wrong-use", "`is_subject` is a trigger", 9);
    expect_report(&reports, "choice", "expected one of", 12);
    expect_report(&reports, "unknown-field", "unexpected_field", 16);

    let missing = JOURNAL_ENTRY
        .replacen("trigger = { always = yes }", "", 1)
        .replacen("header = je_tiger_test_header", "", 1)
        .replacen("effect_header = je_tiger_test_effect_header", "", 1)
        .replace("conclusion_desc = je_tiger_test_conclusion", "");
    let reports = validate(&game, &mod_dir, &missing);
    for field in ["trigger", "header", "effect_header", "conclusion_desc"] {
        expect_report(&reports, "field-missing", &format!("required field `{field}` missing"), 4);
    }
    expect_report(&reports, "field-missing", "required field `conclusion_desc` missing", 11);

    let duplicate_timeout = JOURNAL_ENTRY
        .replace("trigger = { always = yes }", "flags = { timeout } trigger = { always = yes }");
    let reports = validate(&game, &mod_dir, &duplicate_timeout);
    expect_report(&reports, "validation", "outcomes without the `timeout` flag", 8);
    expect_report(&reports, "validation", "only one outcome per journal entry", 12);

    let invalid_outcome = JOURNAL_ENTRY
        .replace(
            "trigger = { always = yes }",
            "flags = { invalid_outcome } trigger = { always = yes }",
        )
        .replace("hidden timeout", "hidden timeout invalid_outcome");
    let reports = validate(&game, &mod_dir, &invalid_outcome);
    expect_report(&reports, "validation", "must also have the `hidden` flag", 8);
    expect_report(&reports, "validation", "only one outcome per journal entry", 12);

    let recovered = validate(&game, &mod_dir, JOURNAL_ENTRY);
    assert!(recovered.is_empty(), "{recovered:#?}");
}

#[test]
fn country_triggers_in_outcomes() {
    let (_fixture, game, mod_dir) = setup();
    write_file(
        &game,
        "game/map_data/state_regions/test_regions.txt",
        "\u{feff}STATE_TIGER_TEST = { id = 1 provinces = {} }",
    );
    write_file(&game, "game/common/country_definitions/test_countries.txt", "\u{feff}TST = {}");
    let native_triggers = JOURNAL_ENTRY.replace(
        "trigger = { always = yes }",
        "trigger = {
            owns_entire_state_region_uncontested = STATE_TIGER_TEST
            war_may_restore_country = TST
            war_may_restore_country = cd:TST
            war_may_restore_country = c:TST.country_definition
            is_independent_and_not_being_subjugated = yes
            country_or_subject_owns_entire_state_region_uncontested = STATE_TIGER_TEST
            has_or_is_contesting_state_in_state_region = STATE_TIGER_TEST
            is_subject_of_and_not_being_freed = c:TST
            owns_entire_or_is_contesting_state_region = STATE_TIGER_TEST
            has_state_in_state_region_uncontested = STATE_TIGER_TEST
        }",
    );
    let valid = validate(&game, &mod_dir, &native_triggers);
    assert!(valid.is_empty(), "{valid:#?}");

    let malformed_triggers = native_triggers
        .replace(
            "owns_entire_state_region_uncontested = STATE_TIGER_TEST",
            "owns_entire_state_region_uncontested = STATE_MISSING",
        )
        .replace("war_may_restore_country = cd:TST", "war_may_restore_country = c:TST")
        .replace(
            "war_may_restore_country = c:TST.country_definition",
            "any_scope_state = { war_may_restore_country = TST }",
        )
        .replace(
            "is_independent_and_not_being_subjugated = yes",
            "is_independent_and_not_being_subjugatd = yes",
        );
    let reports = validate(&game, &mod_dir, &malformed_triggers);
    expect_report(&reports, "missing-item", "STATE_MISSING", 9);
    expect_report(&reports, "scopes", "expected country definition", 11);
    expect_report(&reports, "scopes", "scope seems to be state", 12);
    expect_report(&reports, "unknown-field", "is_independent_and_not_being_subjugatd", 13);

    let repaired = validate(&game, &mod_dir, &native_triggers);
    assert!(repaired.is_empty(), "{repaired:#?}");
}
