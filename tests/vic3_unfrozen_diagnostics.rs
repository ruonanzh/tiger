#![cfg(feature = "vic3")]

use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, LogReportMetadata, Severity, take_reports};

const TRIGGERS: &str = "common/scripted_triggers/review.txt";
const EFFECTS: &str = "common/scripted_effects/review.txt";
const VALUES: &str = "common/script_values/review.txt";
const GUI: &str = "gui/review.gui";
const RULES: &str = "common/game_rules/review.txt";
const LOCA: &str = "localization/english/review_l_english.yml";
const FIXTURE_PATHS: [&str; 6] = [TRIGGERS, EFFECTS, VALUES, GUI, RULES, LOCA];
const LABELS: &str = r#"l_english:
 rule_fixture_rule: "Fixture"
 setting_fixture_setting: "Fixture setting"
 setting_fixture_setting_desc: "Fixture setting description"
"#;

fn validate(root: &Path, inputs: [&str; 6]) -> Vec<(String, u32, LogReportMetadata)> {
    for (path, text) in FIXTURE_PATHS.into_iter().zip(inputs) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if path.extension().is_some_and(|ext| matches!(ext.to_str(), Some("txt" | "yml"))) {
            fs::write(path, format!("\u{feff}{text}")).unwrap();
        } else {
            fs::write(path, text).unwrap();
        }
    }

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            if FIXTURE_PATHS.contains(&path.as_str()) {
                reports.push((path, loc.line, report.clone()));
            }
        }
    }
    reports
}

#[test]
fn vic3_unfrozen_diagnostics_follow_the_semantic_context_and_recover() {
    Game::set(Game::Vic3).unwrap();
    let root = std::env::temp_dir().join(format!("vic3-tiger-review-{}", std::process::id()));
    let buildings = root.join("common/buildings");
    fs::create_dir_all(&buildings).unwrap();
    fs::write(buildings.join("fixture.txt"), "\u{feff}building_example = {}").unwrap();
    let methods = root.join("common/production_methods");
    fs::create_dir_all(&methods).unwrap();
    fs::write(methods.join("fixture.txt"), "\u{feff}pm_fixture = {}").unwrap();
    let bindings = root.join("data_binding");
    fs::create_dir_all(&bindings).unwrap();
    fs::write(
        bindings.join("fixture.txt"),
        "\u{feff}macro = { definition = FixtureReceiver replace_with = runtime_receiver }",
    )
    .unwrap();
    let good = [
        r#"check_year = { year >= 1839 year < fixture_year game_date >= 1839.1.1 }
check_building = { country_has_building_type_levels = { target = bt:building_example value >= 10 } }
check_exports = { state_exports = { value >= 1 } }
check_setting = { has_game_rule = fixture_setting }"#,
        "make_character = { create_character = { culture = primary_culture } }",
        "fixture_year = 1839\nstate_total = { every_scope_state = { add = 1 } }",
        "template fixture { text = \"[GetPlayer.GetName]\" start_sound = {} }",
        "fixture_rule = { default = fixture_setting fixture_setting = { flag = blocks_achievements flag = disable_pm_fixture flag = force_pm_fixture } }",
        LABELS,
    ];
    let clean = validate(&root, good);
    assert!(clean.is_empty(), "{clean:?}");

    let bad = [
        r#"check_inputs = {
    year >= 1839.1.1
    year >= 1839.2.30
    year >= nonexistent_year
    country_has_building_group_levels = { target = bt:building_example value >= 10 }
    country_has_building_group_levels = { target = unexpected value >= 10 }
    state_has_building_group_levels = { target = bt:building_example value >= 10 }
    exists = yes
    exists = no
    has_game_rule = missing_setting
}"#,
        r#"make_character = {
    create_character = { culture = ig_before_random_accepted }
    create_character = { culture = misspelled_culture }
}"#,
        r#"state_total = { every_scope_state = { add = "state_exports" } }
not_state_total = { add = "state_exports" }
misspelled_total = { every_scope_state = { add = "state_export" } }"#,
        r#"template fixture {
    sstart_sound = {}
    strt_sound = {}
    text = "[Scope.GetType.UnknownGuiMember]"
}"#,
        r#"fixture_rule = {
    default = missing_setting
    fixture_setting = { flag = fixture_setting }
    another_setting = { flag = another_setting }
    disable_missing_pm = { flag = disable_missing_pm }
    force_missing_pm = { flag = force_missing_pm }
    other_setting = { flag = unknown_flag }
}"#,
        r#"l_english:
 dynamic_receiver: "[runtime_receiver.UnknownLocaMember]"
 bound_receiver: "[FixtureReceiver.UnknownBoundMember]"
 concrete_receiver: "[GetPlayer.UnknownCountryMember]"
 static_scope: "[SCOPE.sInterestGroup('ig_landowners')]"
 bad_signature: "[GetPlayer.GetName('extra')]"
 unknown_global: "[UnknownGlobalFunction()]"
"#,
    ];
    let reports = validate(&root, bad);
    for (path, line, msg, info) in [
        (TRIGGERS, 2, "year >= 1839.1.1 uses a full date", "game_date >= 1839.1.1"),
        (
            EFFECTS,
            2,
            "create_character.culture = ig_before_random_accepted needs runtime review",
            "character templates",
        ),
        (
            VALUES,
            1,
            "bare state_exports in a State script value needs runtime review",
            "comparison block",
        ),
    ] {
        let at_line: Vec<_> = reports.iter().filter(|(p, l, _)| p == path && *l == line).collect();
        assert_eq!(at_line.len(), 1, "{path}:{line} {at_line:?}");
        let report = &at_line[0].2;
        assert_eq!(report.key.to_string(), "validation");
        assert_eq!(report.severity, Severity::Tips);
        assert_eq!(report.confidence, Confidence::Weak);
        assert_eq!(report.msg, msg);
        assert!(report.info.as_deref().unwrap_or_default().contains(info), "{report:?}");
    }

    for (path, line, key, msg) in [
        (TRIGGERS, 8, "scopes", "`exists = yes/no` needs runtime review"),
        (TRIGGERS, 9, "scopes", "`exists = yes/no` needs runtime review"),
        (RULES, 3, "choice", "game-rule flag matching its setting name needs runtime review"),
        (RULES, 4, "choice", "game-rule flag matching its setting name needs runtime review"),
    ] {
        let report = &reports
            .iter()
            .find(|(p, l, r)| p == path && *l == line && r.key.to_string() == key)
            .unwrap_or_else(|| panic!("{path}:{line} {reports:?}"))
            .2;
        assert_eq!(report.severity, Severity::Tips);
        assert_eq!(report.confidence, Confidence::Weak);
        assert_eq!(report.msg, msg);
    }
    for line in [5, 6, 7] {
        assert!(reports.iter().any(|(p, l, r)| {
            p == RULES
                && *l == line
                && r.key.to_string() == "choice"
                && r.severity == Severity::Error
        }));
    }
    assert!(reports.iter().any(|(p, l, r)| {
        p == RULES && *l == 2 && r.msg == "default value not found among the settings"
    }));
    assert!(
        reports
            .iter()
            .any(|(p, l, r)| { p == TRIGGERS && *l == 10 && r.key.to_string() == "missing-item" })
    );

    for (path, line, msg, expected_severity) in [
        (GUI, 4, "unknown datafunction UnknownGuiMember", Severity::Tips),
        (LOCA, 2, "unknown datafunction UnknownLocaMember", Severity::Tips),
        (LOCA, 3, "unknown datafunction UnknownBoundMember", Severity::Tips),
        (LOCA, 4, "unknown datafunction UnknownCountryMember", Severity::Warning),
        (LOCA, 5, "unknown datafunction sInterestGroup", Severity::Warning),
        (LOCA, 6, "GetName takes 0 arguments but was given 1 here", Severity::Warning),
        (LOCA, 7, "unknown datafunction UnknownGlobalFunction", Severity::Warning),
    ] {
        let report = &reports
            .iter()
            .find(|(p, l, r)| p == path && *l == line && r.msg == msg)
            .unwrap_or_else(|| panic!("{path}:{line} {reports:?}"))
            .2;
        assert_eq!(report.key.to_string(), "datafunctions");
        assert_eq!(report.severity, expected_severity);
        if expected_severity == Severity::Tips {
            assert_eq!(report.confidence, Confidence::Weak);
        }
    }
    let non_calendar: Vec<_> =
        reports.iter().filter(|(p, l, _)| p == TRIGGERS && *l == 3).collect();
    assert_eq!(non_calendar.len(), 1, "{non_calendar:?}");
    let report = &non_calendar[0].2;
    assert_eq!(report.key.to_string(), "validation");
    assert_eq!(report.severity, Severity::Warning);
    assert_eq!(report.msg, "year >= 1839.2.30 uses an invalid calendar date");
    assert!(report.info.as_deref().unwrap_or_default().contains("game_date"), "{report:?}");

    for (path, line) in [(TRIGGERS, 4), (EFFECTS, 3), (VALUES, 2), (VALUES, 3)] {
        assert!(
            reports
                .iter()
                .any(|(p, l, r)| p == path && *l == line && r.severity == Severity::Error),
            "{path}:{line} {reports:?}"
        );
    }
    assert!(reports.iter().any(|(path, _, report)| {
        path == TRIGGERS && report.msg == "required field `type` missing"
    }));
    for (path, line, confidence, suggestion) in [
        (TRIGGERS, 5, Confidence::Weak, Some("country_has_building_type_levels")),
        (TRIGGERS, 6, Confidence::Weak, None),
        (TRIGGERS, 7, Confidence::Weak, None),
        (GUI, 2, Confidence::Strong, Some("Use start_sound")),
        (GUI, 3, Confidence::Weak, None),
    ] {
        let report = &reports
            .iter()
            .find(|(p, l, r)| p == path && *l == line && r.key.to_string() == "unknown-field")
            .unwrap_or_else(|| panic!("{path}:{line} {reports:?}"))
            .2;
        assert_eq!(report.severity, Severity::Error);
        assert_eq!(report.confidence, confidence);
        if let Some(suggestion) = suggestion {
            assert!(report.info.as_deref().unwrap_or_default().contains(suggestion), "{report:?}");
        } else {
            assert!(report.info.is_none(), "{report:?}");
        }
    }

    let recovered = validate(&root, good);
    assert!(recovered.is_empty(), "{recovered:?}");
    fs::remove_dir_all(root).unwrap();
}
