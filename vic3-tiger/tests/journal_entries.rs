use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const ENTRY_PATH: &str = "common/journal_entries/test_outcomes.txt";

const LANGUAGES: [&str; 11] = [
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
];

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
    severity: String,
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

    for lang in LANGUAGES {
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

#[test]
fn population_and_country_outcome_arguments() {
    let (_fixture, game, mod_dir) = setup();
    write_file(&game, "game/common/pop_types/test_pops.txt", "\u{feff}tiger_pop = {}");
    write_file(
        &game,
        "game/common/decrees/test_decrees.txt",
        "\u{feff}decree_tiger_old = {}\ndecree_tiger_new = {}",
    );
    write_file(&game, "game/common/country_definitions/test_countries.txt", "\u{feff}TST = {}");

    let script = JOURNAL_ENTRY
        .replace(
            "trigger = { always = yes }",
            r#"trigger = {
            pop_type_percent_country = { target = pop_type:tiger_pop value > 0.25 }
            "pop_type_percent_country(pop_type:tiger_pop)" >= 0.25
        }"#,
        )
        .replace(
            "effect = { set_variable = tiger_completed }",
            "effect = {
            replace_decree = { from = decree_tiger_old to = decree_tiger_new }
            every_scope_state = { replace_decree = { from = decree_tiger_new to = decree_tiger_old } }
            every_scope_war = { add_war_support_change = { target = c:TST value = -10 } }
            leave_plays_supporting = c:TST
        }",
        );

    let clean = validate(&game, &mod_dir, &script);
    assert!(clean.is_empty(), "{clean:#?}");

    let malformed = script
        .replace("target = pop_type:tiger_pop", "target = pop_type:missing_pop")
        .replace("(pop_type:tiger_pop)", "(pop_type:other_missing_pop)")
        .replace("from = decree_tiger_old to = decree_tiger_new", "to = decree_tiger_new")
        .replace("to = decree_tiger_old", "to = decree_tiger_missing")
        .replace("target = c:TST value = -10", "target = cd:TST value = c:TST")
        .replace("leave_plays_supporting = c:TST", "leave_plays_supporting = cd:TST");

    let reports = validate(&game, &mod_dir, &malformed);
    expect_report(&reports, "missing-item", "missing_pop", 9);
    expect_report(&reports, "missing-item", "other_missing_pop", 10);
    expect_report(&reports, "field-missing", "required field `from` missing", 13);
    expect_report(&reports, "missing-item", "decree_tiger_missing", 14);
    expect_report(&reports, "scopes", "expected country", 15);
    expect_report(&reports, "scopes", "expected value", 15);
    expect_report(&reports, "scopes", "expected country", 16);

    let empty_argument = script.replace("(pop_type:tiger_pop)", "()");
    let reports = validate(&game, &mod_dir, &empty_argument);
    expect_report(&reports, "validation", "empty expression", 10);
    assert_eq!(reports.len(), 1, "{reports:#?}");

    let repaired = validate(&game, &mod_dir, &script);
    assert!(repaired.is_empty(), "{repaired:#?}");
}

#[test]
fn entry_context_overrides_the_group_in_both_directions() {
    let (_fixture, game, mod_dir) = setup();
    write_file(
        &game,
        "game/common/journal_entry_groups/test_groups.txt",
        "\u{feff}global_group = { context = none }\ncountry_group = { context = country }",
    );
    for (group, context, expects_country) in [
        ("global_group", "context = country", true),
        ("global_group", "", false),
        ("country_group", "context = none", false),
        ("country_group", "", true),
    ] {
        let script = format!(
            "je_tiger_test = {{\n group = {group}\n {context}\n possible = {{ is_subject = yes }}\n}}"
        );
        let reports = validate(&game, &mod_dir, &script);
        if expects_country {
            assert!(reports.is_empty(), "{reports:#?}");
        } else {
            expect_report(&reports, "scopes", "scope seems to be none", 4);
        }
    }
    let reports = validate(&game, &mod_dir, "je_tiger_test = {\n context = typo\n}");
    expect_report(&reports, "choice", "expected one of", 2);
}

#[test]
fn subject_play_and_career_length_triggers() {
    let (_fixture, game, mod_dir) = setup();
    write_file(
        &game,
        "game/common/character_roles/test_roles.txt",
        "\u{feff}character_role_favorite = { type = politician }\n",
    );
    let valid = JOURNAL_ENTRY.replace(
        "trigger = { always = yes }",
        "trigger = {
            can_start_play_against_overlord = yes
            can_start_play_against_overlord = no
            ruler = {
                current_career_length = { role = character_role_favorite months = 12 }
                current_career_length = { role = general years = 10 }
                current_career_length = { months = 12 }
            }
        }",
    );
    let reports = validate(&game, &mod_dir, &valid);
    assert!(reports.is_empty(), "{reports:#?}");

    let invalid = valid
        .replace("can_start_play_against_overlord = no", "can_start_play_against_overlord = maybe")
        .replace("role = general years = 10", "role = nonexistent years = 10")
        .replace("current_career_length = { months = 12 }", "current_career_length = { }");
    let reports = validate(&game, &mod_dir, &invalid);
    expect_report(&reports, "unknown-field", "unknown token `maybe`", 10);
    expect_report(
        &reports,
        "missing-item",
        "not found as character role or character archetype",
        13,
    );
    expect_report(&reports, "field-missing", "must have 1 of", 14);
}

#[test]
fn journal_entry_modifier_multiplier_scope() {
    let (_fixture, game, mod_dir) = setup();
    write_file(
        &game,
        "game/common/static_modifiers/test_modifier.txt",
        "\u{feff}test_modifier = {}\n",
    );
    let script = JOURNAL_ENTRY.replacen(
        "effect = { set_variable = tiger_completed }",
        r"effect = {
            add_modifier = {
                name = test_modifier
                multiplier = scope:journal_entry.var:tiger_value
            }
            ruler = {
                add_modifier = {
                    name = test_modifier
                    multiplier = { value = scope:journal_entry.var:tiger_value }
                }
            }
        }",
        1,
    );
    let reports = validate(&game, &mod_dir, &script);
    assert!(reports.is_empty(), "{reports:#?}");

    let typo = script.replacen(
        "scope:journal_entry.var:tiger_value",
        "scope:journal_entyr.var:tiger_value",
        1,
    );
    let reports = validate(&game, &mod_dir, &typo);
    expect_report(&reports, "strict-scopes", "scope:journal_entyr might not be available here", 12);

    let outside_path = "events/batch4_multiplier.txt";
    write_file(
        &mod_dir,
        outside_path,
        "\u{feff}namespace = batch4_multiplier
batch4_multiplier.1 = {
    type = country_event
    hidden = yes
    immediate = {
        add_modifier = {
            name = test_modifier
            multiplier = scope:journal_entry.var:tiger_value
        }
    }
}",
    );
    let reports = validate(&game, &mod_dir, &script);
    assert!(
        reports.iter().any(|report| {
            report.key == "strict-scopes"
                && report.message.contains("scope:journal_entry might not be available here")
                && report.locations.iter().any(|location| {
                    location.from == "MOD"
                        && location.path.replace('\\', "/") == outside_path
                        && location.linenr == 8
                })
        }),
        "missing strict-scope diagnostic for the unrelated event multiplier: {reports:#?}"
    );
}

#[test]
fn outcome_scopes_from_unconditional_immediate_saves() {
    let (_fixture, game, mod_dir) = setup();
    write_file(&game, "game/common/country_definitions/test_countries.txt", "\u{feff}GBR = {}");

    let valid = JOURNAL_ENTRY
        .replace(
            "timeout = 30",
            "timeout = 30
    immediate = {
        c:GBR = { save_scope_as = je_saved_tag }
        root = { save_scope_as = je_saved_root }
        save_scope_as = je_saved_current
    }",
        )
        .replace(
            "trigger = { always = yes }",
            "trigger = {
            scope:je_saved_tag ?= { is_subject = yes }
            is_subject_of_and_not_being_freed = scope:je_saved_root
            is_subject_of_and_not_being_freed = scope:je_saved_current
        }",
        )
        .replace("set_variable = tiger_timed_out", "leave_plays_supporting = scope:je_saved_root");

    let reports = validate(&game, &mod_dir, &valid);
    assert!(reports.is_empty(), "{reports:#?}");

    let wrong_type = valid.replace(
        "is_subject_of_and_not_being_freed = scope:je_saved_root",
        "war_may_restore_country = scope:je_saved_root",
    );
    let reports = validate(&game, &mod_dir, &wrong_type);
    expect_report(&reports, "scopes", "expected country definition", 15);

    let restored = validate(&game, &mod_dir, &valid);
    assert!(restored.is_empty(), "{restored:#?}");
}

#[test]
fn outcome_scopes_do_not_inherit_dynamic_or_unrelated_saves() {
    let (_fixture, game, mod_dir) = setup();
    write_file(&game, "game/common/country_definitions/test_countries.txt", "\u{feff}GBR = {}");
    for lang in LANGUAGES {
        write_file(
            &mod_dir,
            &format!("localization/{lang}/isolation_l_{lang}.yml"),
            &format!(
                "\u{feff}l_{lang}:\n je_tiger_other: \"Other entry\"\n je_tiger_other_reason: \"Other reason\""
            ),
        );
    }

    let main = JOURNAL_ENTRY
        .replace(
            "timeout = 30",
            "timeout = 30
    immediate = {
        if = {
            limit = { is_subject = yes }
            ruler = { save_scope_as = je_saved_conditional }
        }
        every_scope_state = { save_scope_as = je_saved_iterator }
        c:GBR = { save_scope_as = je_saved_tag }
        save_scope_as = je_saved_cleared
        clear_saved_scope = je_saved_cleared
        save_temporary_scope_as = je_saved_temporary
        save_scope_as = je_saved_overwritten
        if = {
            limit = { is_subject = yes }
            capital = { save_scope_as = je_saved_overwritten }
        }
    }",
        )
        .replace(
            "trigger = { always = yes }",
            "trigger = {
            is_subject_of_and_not_being_freed = scope:je_saved_conditional
            is_subject_of_and_not_being_freed = scope:je_saved_iterator
            is_subject_of_and_not_being_freed = scope:je_saved_missing
            is_subject_of_and_not_being_freed = scope:je_saved_other
            is_subject_of_and_not_being_freed = scope:je_saved_tag
            is_subject_of_and_not_being_freed = scope:je_saved_cleared
            is_subject_of_and_not_being_freed = scope:je_saved_temporary
            scope:je_saved_overwritten ?= { is_subject = yes }
        }",
        )
        .replace(
            "effect = { set_variable = tiger_completed }",
            "effect = { ruler = { save_scope_as = je_saved_sibling } }",
        )
        .replace(
            "effect = { set_variable = tiger_timed_out }",
            "effect = {
            leave_plays_supporting = scope:je_saved_sibling
            if = {
                limit = { scope:je_saved_overwritten ?= { is_incorporated = yes } }
                set_variable = tiger_visited
            }
            if = {
                limit = { exists = scope:je_saved_guarded }
                leave_plays_supporting = scope:je_saved_guarded
            }
        }",
        );
    let other = "je_tiger_other = {
    immediate = { ruler = { save_scope_as = je_saved_other } }
}
";

    for script in [format!("{main}{other}"), format!("{other}{main}")] {
        let reports = validate(&game, &mod_dir, &script);
        for name in [
            "je_saved_conditional",
            "je_saved_iterator",
            "je_saved_missing",
            "je_saved_other",
            "je_saved_sibling",
            "je_saved_tag",
            "je_saved_cleared",
            "je_saved_temporary",
        ] {
            let marker = format!("= scope:{name}");
            let line = script.lines().position(|line| line.contains(&marker)).unwrap() + 1;
            expect_report(
                &reports,
                "strict-scopes",
                &format!("scope:{name} might not be available here"),
                line,
            );
        }
        assert_eq!(reports.len(), 8, "{reports:#?}");
        assert!(
            reports.iter().all(|report| {
                matches!(report.severity.as_str(), "tips" | "untidy" | "warning")
            }),
            "{reports:#?}"
        );
    }
}

#[test]
fn global_journal_entry_activation_scopes_reach_involved_outcomes() {
    let (_fixture, game, mod_dir) = setup();
    write_file(&game, "game/common/country_definitions/test_countries.txt", "\u{feff}GBR = {}");

    let valid = r"je_tiger_test = {
    context = none
    timeout = 30
    immediate = {
        scope:journal_entry = { save_scope_as = je_saved_entry }
    }
    immediate_all_involved = { save_scope_as = je_saved_participant }
    outcome = {
        flags = { hidden timeout }
        conclusion_desc = je_tiger_test_conclusion
        effect = {
            scope:je_saved_entry = { add_involved_country = c:GBR }
        }
        effect_all_involved = {
            scope:je_saved_entry = { add_involved_country = root }
            leave_plays_supporting = root
        }
    }
}
";
    let reports = validate(&game, &mod_dir, valid);
    assert!(reports.is_empty(), "{reports:#?}");

    let broken = valid
        .replace("add_involved_country = c:GBR", "leave_plays_supporting = c:GBR")
        .replace("leave_plays_supporting = root", "leave_plays_supporting = scope:je_saved_entry")
        .replace(
            "effect = {",
            "effect = {
            scope:je_saved_participant = { leave_plays_supporting = c:GBR }",
        );
    let reports = validate(&game, &mod_dir, &broken);
    expect_report(
        &reports,
        "strict-scopes",
        "scope:je_saved_participant might not be available here",
        12,
    );
    expect_report(&reports, "scopes", "scope seems to be journal entry", 13);
    expect_report(&reports, "scopes", "produces journal entry but expected country", 17);
}

#[test]
fn outcome_scopes_from_optional_root_and_custom_tooltip() {
    let (_fixture, game, mod_dir) = setup();
    let valid = JOURNAL_ENTRY
        .replace(
            "timeout = 30",
            "timeout = 30
    immediate = {
        root ?= { save_scope_as = je_saved_optional_root }
        custom_tooltip = {
            text = je_tiger_test_reason
            save_scope_as = je_saved_tooltip
        }
    }",
        )
        .replace(
            "trigger = { always = yes }",
            "trigger = {
            is_subject_of_and_not_being_freed = scope:je_saved_optional_root
            is_subject_of_and_not_being_freed = scope:je_saved_tooltip
        }",
        );

    let reports = validate(&game, &mod_dir, &valid);
    assert!(reports.is_empty(), "{reports:#?}");

    let broken = valid.replace(
        "is_subject_of_and_not_being_freed = scope:je_saved_",
        "war_may_restore_country = scope:je_saved_",
    );
    let reports = validate(&game, &mod_dir, &broken);
    assert_eq!(reports.len(), 2, "{reports:#?}");
    for line in [16, 17] {
        expect_report(&reports, "scopes", "produces country but expected country definition", line);
    }
}

#[test]
fn uncertain_je_scope_diagnostics_follow_event_calls() {
    let (_fixture, game, mod_dir) = setup();
    write_file(
        &game,
        "game/common/static_modifiers/test_modifier.txt",
        "\u{feff}test_modifier = {}\n",
    );
    let events = "events/test_je_event_scopes.txt";
    write_file(
        &mod_dir,
        events,
        "\u{feff}namespace = je_test_events
je_test_events.1 = {
    type = country_event
    hidden = yes
    immediate = {
        leave_plays_supporting = scope:uncertain_je_name
    }
}
je_test_events.2 = {
    type = country_event
    hidden = yes
    immediate = {
        add_modifier = { name = test_modifier multiplier = scope:uncertain_je_name.var:tiger_value }
    }
}
",
    );
    let script = JOURNAL_ENTRY.replace(
        "timeout = 30",
        "timeout = 30
    immediate = { trigger_event = je_test_events.1 }",
    );
    let reports = validate(&game, &mod_dir, &script);
    for (line, severity) in [(6, "warning"), (13, "error")] {
        assert!(
            reports.iter().any(|report| {
                report.key == "strict-scopes"
                    && report.severity == severity
                    && report
                        .message
                        .contains("scope:uncertain_je_name might not be available here")
                    && report.locations.first().is_some_and(|location| {
                        location.from == "MOD"
                            && location.path.replace('\\', "/") == events
                            && location.linenr == line
                    })
            }),
            "missing {severity} diagnostic at {events}:{line}\n{reports:#?}"
        );
    }
    assert_eq!(reports.len(), 2, "{reports:#?}");
}
