use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

const EVENTS: &str = "events/scope_fixture.txt";

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_modifier_multipliers_with_their_script_context() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(
        root,
        "common/journal_entries/scope_fixture.txt",
        "je_scope_fixture = {
            context = country
            immediate = { trigger_event = scope_fixture.1 trigger_event = scope_fixture.4 }
        }",
    );
    write(
        root,
        "common/on_actions/scope_fixture.txt",
        "on_law_checkpoint_stall = { events = { scope_fixture.2 scope_fixture.4 } }",
    );
    write(root, "common/cultures/scope_fixture.txt", "fixture_culture = {}");

    let events = r#"namespace = scope_fixture
scope_fixture.1 = {
    type = country_event
    hidden = yes
    immediate = {
        capital = {
            add_modifier = {
                name = fixture_modifier
                multiplier = { value = "scope:journal_entry.scripted_bar_progress(fixture_bar)" }
            }
        }
    }
}
scope_fixture.2 = {
    type = country_event
    immediate = { cu:fixture_culture = { save_scope_as = fixture_culture } }
    option = {
        add_enactment_modifier = {
            name = fixture_modifier
            multiplier = "culture_percent_country(scope:fixture_culture)"
        }
    }
}
scope_fixture.3 = {
    type = country_event
    hidden = yes
    immediate = {
        add_modifier = {
            name = fixture_modifier
            multiplier = { value = "scope:journal_entry.scripted_bar_progress(fixture_bar)" }
        }
    }
}
scope_fixture.4 = {
    type = country_event
    hidden = yes
    immediate = {
        add_modifier = {
            name = fixture_modifier
            multiplier = { value = "scope:journal_entry.scripted_bar_progress(fixture_bar)" }
        }
    }
}
"#;

    for broken in [false, true, false] {
        let source = if broken {
            events
                .replacen("scope:journal_entry", "scope:journal_entrry", 1)
                .replace("scope:fixture_culture", "scope:fixture_culturre")
        } else {
            events.to_owned()
        };
        write(root, EVENTS, &source);
        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut unavailable = Vec::new();
        for (report, occurrences) in take_reports() {
            if report.key.to_string() != "strict-scopes" {
                continue;
            }
            for pointers in occurrences {
                let loc = &pointers[0].loc;
                if loc.pathname() == Path::new(EVENTS) {
                    unavailable.push((loc.line, report.msg.clone()));
                }
            }
        }
        let lines = |name: &str| {
            let mut lines: Vec<_> = unavailable
                .iter()
                .filter(|(_, msg)| msg.starts_with(&format!("scope:{name} ")))
                .map(|(line, _)| *line)
                .collect();
            lines.sort_unstable();
            lines.dedup();
            lines
        };
        // The fourth event also has an explicit country caller without a journal entry.
        assert_eq!(lines("journal_entry"), [30, 40]);
        assert!(lines("fixture_culture").is_empty(), "{unavailable:?}");
        assert_eq!(lines("journal_entrry"), if broken { &[9][..] } else { &[] });
        assert_eq!(lines("fixture_culturre"), if broken { &[20][..] } else { &[] });
    }
}
