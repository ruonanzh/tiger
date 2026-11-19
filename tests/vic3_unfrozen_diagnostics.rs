#![cfg(feature = "vic3")]

use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, LogReportMetadata, Severity, take_reports};

const TRIGGERS: &str = "common/scripted_triggers/review.txt";
const EFFECTS: &str = "common/scripted_effects/review.txt";
const VALUES: &str = "common/script_values/review.txt";
const GUI: &str = "gui/review.gui";

fn validate(root: &Path, inputs: [&str; 4]) -> Vec<(String, u32, LogReportMetadata)> {
    for (path, text) in [TRIGGERS, EFFECTS, VALUES, GUI].into_iter().zip(inputs) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if path.extension().is_some_and(|ext| ext == "txt") {
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
            if [TRIGGERS, EFFECTS, VALUES, GUI].contains(&path.as_str()) {
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
    let good = [
        r#"check_year = { year >= 1839 year < fixture_year game_date >= 1839.1.1 }
check_building = { country_has_building_type_levels = { target = bt:building_example value >= 10 } }
check_exports = { state_exports = { value >= 1 } }"#,
        "make_character = { create_character = { culture = primary_culture } }",
        "fixture_year = 1839\nstate_total = { every_scope_state = { add = 1 } }",
        "template fixture { start_sound = {} }",
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
}"#,
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

    for (path, line) in [(TRIGGERS, 3), (TRIGGERS, 4), (EFFECTS, 3), (VALUES, 2), (VALUES, 3)] {
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
    for (path, line, suggestion) in [
        (TRIGGERS, 5, Some("country_has_building_type_levels")),
        (TRIGGERS, 6, None),
        (TRIGGERS, 7, None),
        (GUI, 2, Some("Use start_sound")),
        (GUI, 3, None),
    ] {
        let report = &reports
            .iter()
            .find(|(p, l, r)| p == path && *l == line && r.key.to_string() == "unknown-field")
            .unwrap_or_else(|| panic!("{path}:{line} {reports:?}"))
            .2;
        assert_eq!(report.severity, Severity::Error);
        assert_eq!(report.confidence, Confidence::Weak);
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
