use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

const JOURNAL: &str = "common/journal_entries/display_fixture.txt";
const MODIFIER: &str = "common/static_modifiers/display_fixture.txt";

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn display_only_effects_validate_without_applying_modifiers() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let journal = r#"je_display_fixture = {
    context = country
    immediate = {
        wrapped_display = yes # missing_input
        capital = {
            save_scope_as = caller_context
            wrapped_display = yes # wrong_input
            show_as_tooltip = {
                save_scope_as = preview_fixture
                scope:preview_fixture = { add_modifier = fixture_modifier } # tooltip
                add_modifier = missing_fixture
            }
            add_modifier = fixture_modifier # executed
            hidden_effect = { add_modifier = fixture_modifier } # hidden
            scope:preview_fixture = {} # unavailable
        }
    }
}
je_global_display_fixture = {
    context = none
    outcome = {
        header = fixture_header
        effect_header = fixture_header
        conclusion_desc = fixture_header
        trigger = { always = yes }
        effect = { add_modifier = fixture_modifier } # global_executed
        event_outcome_effect_desc = {
            effect = {
                add_modifier = fixture_modifier # outcome_preview
                add_modifier = { name = fixture_modifier is_decaying = invalid } # invalid
            }
        }
    }
}"#;
    write(root, JOURNAL, journal);
    write(root, MODIFIER, "fixture_modifier = { country_law_enactment_speed_mult = 0.1 }");

    write(
        root,
        "common/modifier_type_definitions/display_fixture.txt",
        "country_law_enactment_speed_mult = {}",
    );
    write(
        root,
        "common/scripted_effects/display_fixture.txt",
        "wrapped_display = {
            show_as_tooltip = {
                scope:caller_context = { add_loyalists = { value = 0.1 } }
            }
        }",
    );

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let marked_line = |marker: &str| {
        let suffix = format!("# {marker}");
        u32::try_from(journal.lines().position(|line| line.ends_with(&suffix)).unwrap() + 1)
            .unwrap()
    };
    let mut propagation_lines = Vec::new();
    let mut preview_scope_checked = false;
    let mut missing_modifier_checked = false;
    let mut invalid_parameter_checked = false;
    let mut saved_scope_isolated = false;
    let mut missing_input_checked = false;
    let mut wrong_input_checked = false;
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let key = report.key.to_string();
            let pointer = match key.as_str() {
                "modifiers" => pointers.get(1),
                _ => pointers.first(),
            };
            let Some(pointer) = pointer.filter(|p| p.loc.pathname() == Path::new(JOURNAL)) else {
                continue;
            };
            match key.as_str() {
                "modifiers" => propagation_lines.push(pointer.loc.line),
                "scopes" => {
                    preview_scope_checked |= pointer.loc.line == marked_line("outcome_preview");
                    wrong_input_checked |= pointer.loc.line == marked_line("wrong_input")
                        && report.msg.contains("scope:caller_context");
                }
                "missing-item" => {
                    missing_modifier_checked |= report.msg.contains("modifier missing_fixture ")
                }
                "validation" => {
                    invalid_parameter_checked |= pointer.loc.line == marked_line("invalid")
                }
                "strict-scopes" => {
                    saved_scope_isolated |= pointer.loc.line == marked_line("unavailable");
                    missing_input_checked |= pointer.loc.line == marked_line("missing_input")
                        && report.msg.contains("scope:caller_context");
                }
                _ => {}
            }
        }
    }
    propagation_lines.sort_unstable();
    propagation_lines.dedup();
    assert_eq!(
        propagation_lines,
        [marked_line("executed"), marked_line("hidden"), marked_line("global_executed")]
    );
    assert!(preview_scope_checked);
    assert!(missing_modifier_checked);
    assert!(invalid_parameter_checked);
    assert!(saved_scope_isolated);
    assert!(missing_input_checked, "scripted preview should require its input");
    assert!(wrong_input_checked, "scripted preview should type-check its input");
}
