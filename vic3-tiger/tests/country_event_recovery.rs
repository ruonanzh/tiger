use std::path::Path;

use tiger_lib::{Everything, Game, Severity, take_reports};

#[test]
fn country_event_body_uses_declared_scope_after_missing_caller_scope() {
    Game::set(Game::Vic3).unwrap();
    let mod_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vic3-country-event-recovery");
    let mut data = Everything::new(None, None, None, None, &mod_root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let diagnostics: Vec<_> = reports
        .iter()
        .flat_map(|(report, locations)| {
            locations.iter().map(move |pointers| (report, &pointers[0].loc))
        })
        .collect();
    let has_warning = |file, line, text| {
        diagnostics.iter().any(|(report, loc)| {
            report.severity == Severity::Warning
                && loc.pathname() == file
                && loc.line == line
                && report.msg.contains(text)
        })
    };

    let journal = "common/journal_entries/country_event_recovery.txt";
    let events = "events/country_event_recovery.txt";
    let modifiers = "common/static_modifiers/country_event_recovery.txt";
    assert!(has_warning(journal, 7, "`recovery.1` is for country but scope seems to be none"));
    assert!(has_warning(journal, 8, "scope seems to be none"));
    assert!(has_warning(modifiers, 2, "will not flow from none scope"));
    assert!(has_warning(
        events,
        6,
        "`kill_character` is for character but scope seems to be country"
    ));
    assert!(!diagnostics.iter().any(|(report, loc)| {
        loc.pathname() == events && report.msg.contains("scope seems to be none")
    }));
    assert_eq!(
        diagnostics
            .iter()
            .filter(|(report, loc)| {
                loc.pathname() == modifiers && report.msg.contains("will not flow from none scope")
            })
            .count(),
        1,
        "only the direct journal-entry modifier should fail to flow",
    );
    assert!(!diagnostics.iter().any(|(report, loc)| {
        loc.pathname() == events && report.msg.contains("scope:recovery_entry")
    }));
    assert!(
        !diagnostics
            .iter()
            .any(|(report, _)| { report.msg.contains("`recovery.2` is for country") })
    );
}
