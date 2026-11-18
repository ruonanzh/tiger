use std::fs;
use std::path::Path;
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

#[test]
fn journal_entry_outcomes() {
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
