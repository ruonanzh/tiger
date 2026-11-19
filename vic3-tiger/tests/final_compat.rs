use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

fn write(root: &Path, relative: &str, text: &str) {
    let file = root.join(relative);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, format!("\u{feff}{text}")).unwrap();
}

const VANILLA_BUTTONS: &str = "common/scripted_buttons/00_compat.txt";
const BUTTONS: &str = "common/scripted_buttons/final_compat.txt";
const BARS: &str = "common/scripted_progress_bars/final_compat.txt";
const GUI: &str = "gui/final_compat.gui";

const BUTTON_CASES: &[(&str, &str, &[&str])] = &[
    (
        "base_button = { name = fixture_title ai_chance = { base = 5 } }",
        "base_button = { name = fixture_title ai_chance = { base = { value = 5 } } }",
        &["dynamic script values are not allowed here"],
    ),
    (
        "modifier_button = { name = fixture_title ai_chance = { base = 0 modifier = { trigger = { bureaucracy > 0 } add = 5 } } }",
        "modifier_button = { name = fixture_title ai_chance = { base = 0 modifier = { trigger = { bureaucracy > 0 } add = 5 typo = yes } } }",
        &["unknown field"],
    ),
    (
        "script_button = { name = fixture_title ai_chance = { value = 0 if = { limit = { always = yes } add = 5 } } }",
        "script_button = { name = fixture_title ai_chance = { value = 0 if = { limit = { alwayz = yes } add = 5 } } }",
        &["alwayz"],
    ),
    (
        "mixed_button = { name = fixture_title ai_chance = { base = 0 if = { limit = { exists = scope:journal_entry } add = bureaucracy } } }",
        "mixed_button = { name = fixture_title ai_chance = { base = 0 if = { limit = { exists = scope:journal_entry } add = scope:journal_entry } typo = yes } }",
        &["journal", "unknown token `typo`"],
    ),
    (
        "scalar_button = { name = fixture_title ai_chance = 5 }",
        "scalar_button = { name = fixture_title ai_chance = missing_button_value }",
        &["missing_button_value"],
    ),
    (
        "attitude_button = { name = fixture_title possible = { has_attitude = { who = root attitude >= cautious } } }",
        "attitude_button = { name = fixture_title possible = { has_attitude = { who = scope:journal_entry attitude >= cautiouz } } }",
        &["cautiouz", "journal"],
    ),
    (
        "stance_button = { name = fixture_title possible = { ruler = { law_enactment_stance = { law = law_type:fixture_law value > 0 } } } }",
        "stance_button = { name = fixture_title possible = { ruler = { law_enactment_stance = { value > nonsense } } } }",
        &["law", "nonsense"],
    ),
    (
        "names_button = { name = fixture_title effect = { create_character = { template = fixture_person first_name = fixture_title } create_character = { last_name = fixture_title } } }",
        "names_button = { name = fixture_title effect = { create_character = { template = fixture_person first_name = missing_first_name } create_character = { name = fixture_title last_name = fixture_title } } }",
        &["missing_first_name", "last_name"],
    ),
    (
        "INJECT:injected_button = { ai_chance = { base = 2 } }",
        "INJECT:injected_button = { ai_chance = invalid_injected_value }",
        &["invalid_injected_value"],
    ),
    (
        "unique_button = { name = fixture_title ai_chance = 1 }",
        "unique_button = { name = fixture_title ai_chance = 1 ai_chance = 2 }",
        &["ai_chance` is redefined"],
    ),
];

// A named script value and an inline calculation both use the progress bar's JE root.
const BAR_CASES: &[(&str, &str, &[&str])] = &[
    (
        "constant_bar = { name = fixture_title desc = fixture_title default = yes start_value = 50 min_value = 0 max_value = 100 }",
        "constant_bar = { name = fixture_title desc = fixture_title default = yes start_value = fixture_bound min_value = 0 max_value = 100 }",
        &["number"],
    ),
    (
        "dynamic_bar = { name = fixture_title desc = fixture_title default = yes min_value = fixture_bound max_value = { value = journal_entry_age add = 100 } }",
        "dynamic_bar = { name = fixture_title desc = fixture_title default = yes min_value = missing_fixture_bound max_value = { value = bureaucracy } }",
        &["missing_fixture_bound", "bureaucracy"],
    ),
];

const GUI_CASES: &[(&str, &str, &[&str])] = &[
    (
        "widget = { size { 100% 20 } margin { 5 5 } position { 1 2 } }",
        "widget = { size { 20 } margin { none 5 } position { 1 2 3 } }",
        &["expected 2 numeric or percentage components", "number"],
    ),
    (
        "widget = { size { 100 20 } visible = yes }",
        "widget = { szie { 100 20 } visible = maybe }",
        &["unexpected value", "expected yes or no"],
    ),
    (
        r##"textbox = { text "[fixture_concept]" default_format "#v" }"##,
        r##"textbox = { text "[fixture_concepz]" default_format "#v" }"##,
        &["unknown datafunction fixture_concepz"],
    ),
    (
        r#"textbox = { text "[fixture_concept_desc]" }"#,
        r#"textbox = { text "[fixture_concept('anything')]" }"#,
        &["game concept fixture_concept must be used without parentheses"],
    ),
    (
        r#"textbox = { text "[fixture_concept]" }"#,
        r#"textbox = { text "[fixture_concept()]" }"#,
        &["game concept fixture_concept must be used without parentheses"],
    ),
    (
        r#"textbox = { text "[GetPlayer.GetName]" }"#,
        r#"textbox = { text "[GetPlayer.fixture_concept]" }"#,
        &["unknown datafunction fixture_concept"],
    ),
    (
        r#"textbox = { text "Printed text" }"#,
        "textbox = { text unquoted visible = maybe }",
        &["unexpected value", "expected yes or no"],
    ),
    (
        "icon = { mipmaplodbias = 1.8 } icon = { mipmaplodbias = 1.5 } icon = { mipmaplodbias = -1 }",
        "icon = { mipmaplodbias = invalid }",
        &["number"],
    ),
    (
        "widget = { block body { size { 40 50 } } blockoverride body { margin { 1 2 } } }",
        "widget = { block body { size { 40 50 } } blockoverride body { margin { 1 } } }",
        &["expected 2 numeric or percentage components"],
    ),
];

#[test]
fn validates_vic3_final_compatibility_shapes_and_recovers_after_corrections() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let vanilla = tempfile::tempdir().unwrap();
    for directory in ["clausewitz", "jomini"] {
        fs::create_dir_all(vanilla.path().join(directory)).unwrap();
    }
    write(
        &vanilla.path().join("game"),
        VANILLA_BUTTONS,
        "injected_button = { name = fixture_title ai_chance = 1 }",
    );
    write(root, "common/character_templates/final_compat.txt", "fixture_person = {}");
    write(root, "common/laws/final_compat.txt", "fixture_law = {}");
    write(root, "common/game_concepts/final_compat.txt", "fixture_concept = {}");
    write(root, "common/script_values/final_compat.txt", "fixture_bound = journal_entry_age");
    write(
        root,
        "localization/english/final_compat_l_english.yml",
        r#"l_english:
 fixture_title: "Title"
 fixture_concept: "Concept"
 fixture_concept_desc: "Concept description"
 fixture_rendered_concept: "[fixture_concept]"
"#,
    );

    let paths = [(BUTTONS, BUTTON_CASES), (BARS, BAR_CASES), (GUI, GUI_CASES)];
    for malformed in [false, true, false] {
        for (path, cases) in paths {
            let source = cases
                .iter()
                .map(|(good, bad, _)| if malformed { *bad } else { *good })
                .collect::<Vec<_>>()
                .join("\n");
            write(root, path, &source);
        }

        let mut data =
            Everything::new(None, Some(vanilla.path()), None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut reports = Vec::new();
        for (report, occurrences) in take_reports() {
            for pointers in occurrences {
                let loc = pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                if path == VANILLA_BUTTONS || paths.iter().any(|(fixture, _)| path == *fixture) {
                    reports.push((path, loc.line, report.key.to_string(), report.msg.clone()));
                }
            }
        }
        let note = "scripted button AI chance combines weight fields with script-value operations";
        assert!(
            reports.iter().any(|(path, line, key, msg)| {
                path == BUTTONS && *line == 4 && key == "logic" && msg == note
            }),
            "missing mixed-weight compatibility note in {reports:?}"
        );
        reports.retain(|(_, _, _, msg)| msg != note);

        if malformed {
            for (path, cases) in paths {
                for (index, (_, _, fragments)) in cases.iter().enumerate() {
                    for fragment in *fragments {
                        let line = (index + 1) as u32;
                        assert!(
                            reports.iter().any(|(p, n, _, msg)| {
                                p == path && *n == line && msg.contains(fragment)
                            }),
                            "missing {path}:{line} {fragment} in {reports:?}"
                        );
                    }
                }
            }
        } else {
            assert!(reports.is_empty(), "{reports:?}");
        }
    }
}
