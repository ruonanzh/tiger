#![cfg(feature = "ck3")]

use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

#[test]
fn ck3_gui_keeps_original_assignment_and_mipmap_rules() {
    Game::set(Game::Ck3).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/files/ck3-gui-compat");
    let relative = "gui/final_compat.gui";
    let mut data = Everything::new(None, None, None, None, &fixture, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            if loc.pathname().to_string_lossy().replace('\\', "/") == relative {
                reports.push((loc.line, report.msg.clone()));
            }
        }
    }
    assert!(reports.iter().all(|(line, _)| (2..=4).contains(line)), "{reports:?}");
    for (line, fragment) in [(2, "unexpected value"), (3, "unexpected value"), (4, "integer")] {
        assert!(reports.iter().any(|(n, msg)| *n == line && msg.contains(fragment)), "{reports:?}");
    }
}
