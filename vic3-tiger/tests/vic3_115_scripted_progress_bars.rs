use std::path::Path;

use tiger_lib::{Everything, take_reports};

#[test]
fn validates_scripted_progress_bar_looks() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/files/vic3-progress-bars");
    let mut data = Everything::new(None, None, None, None, &fixture, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let mut messages: Vec<_> = reports
        .iter()
        .flat_map(|(report, occurrences)| {
            occurrences.iter().filter_map(move |pointers| {
                let path = pointers[0].loc.pathname();
                path.starts_with("common/scripted_progress_bars/").then_some((
                    path,
                    pointers[0].loc.line,
                    report.msg.as_str(),
                ))
            })
        })
        .collect();
    messages.sort_unstable();

    let invalid = Path::new("common/scripted_progress_bars/invalid.txt");
    let presets = "default, default_green, default_bad, double_sided_gold, double_sided_bad, double_sided_two_spains";
    let exclusive = format!("expected exactly 1 of {presets}");
    let mixed = format!("cannot combine fields {presets}");
    assert_eq!(
        messages,
        [
            (invalid, 1, exclusive.as_str()),
            (invalid, 8, mixed.as_str()),
            (invalid, 16, exclusive.as_str()),
            (invalid, 24, "expected yes or no"),
            (invalid, 25, "expected 4 numbers"),
            (invalid, 26, "file gfx/missing_fill.dds does not exist"),
            (invalid, 27, "file gfx/missing_background.dds does not exist"),
            (invalid, 28, "file gfx/missing_glow.dds does not exist"),
            (invalid, 29, "file gfx/missing_glow_line.dds does not exist"),
            (invalid, 30, "file gfx/missing_left.dds does not exist"),
            (invalid, 31, "file gfx/missing_right.dds does not exist"),
        ],
    );
}
