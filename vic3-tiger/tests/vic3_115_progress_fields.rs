use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

fn write_fixture(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_vic3_cold_effects_progress_conditions_and_journal_dlc() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let states = "map_data/state_regions/schema.txt";
    let bar = "common/scripted_progress_bars/schema.txt";
    let entries = "common/journal_entries/schema.txt";
    let values = "common/script_values/schema.txt";
    write_fixture(
        root,
        "common/strategic_regions/schema.txt",
        "region_test = { states = { STATE_TEST } }",
    );
    write_fixture(root, "dlc_metadata/schema.txt", "dlc_test_a = {}\ndlc_test_b = {}");

    let valid = [
        (
            states,
            "STATE_TEST = {\n    id = 1\n    provinces = {}\n    min_cold_effect = 0.2\n    max_cold_effect = 0.2\n}\n",
        ),
        (
            bar,
            r"test_bar = {
    name = test_bar_name
    desc = test_bar_desc
    default = yes
    monthly_progress = {
        progress_condition = {
            limit = { is_subject = yes }
            add = { value = 2 }
            else = {
                limit = { is_subject = no }
                add = { value = 1 }
                else = { add = { value = 0 } }
            }
        }
    }
}
",
        ),
        (
            entries,
            "je_test_single = {\n    dlc = dlc_test_a\n}\nje_test_list = {\n    dlc = { dlc_test_a dlc_test_b }\n}\n",
        ),
        (values, "test_value = {\n    if = { limit = { always = yes } add = 1 }\n}\n"),
    ];
    let invalid = [
        valid[0].1.replace("max_cold_effect = 0.2", "max_cold_effect = 0.1"),
        valid[1]
            .1
            .replace("is_subject = yes", "set_variable = invalid_trigger")
            .replace("else = { add =", "else = { ad ="),
        valid[2]
            .1
            .replace("dlc = dlc_test_a", "dlc = dlc_missing_single")
            .replace("dlc_test_a dlc_test_b", "dlc_test_a dlc_missing_list"),
        valid[3].1.replace("if =", "progress_condition ="),
    ];
    let expected = [
        (states, 5, "max_cold_effect must be at least min_cold_effect"),
        (bar, 7, "`set_variable` is an effect"),
        (bar, 12, "unknown token `ad`"),
        (entries, 2, "dlc_missing_single"),
        (entries, 5, "dlc_missing_list"),
        (values, 2, "unknown token `progress_condition`"),
    ];

    for malformed in [false, true, false] {
        for (index, (path, contents)) in valid.iter().enumerate() {
            write_fixture(root, path, if malformed { &invalid[index] } else { contents });
        }
        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();
        let mut observed = Vec::new();
        for (report, locations) in take_reports() {
            for pointers in locations {
                let loc = &pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                // The synthetic objects have no localization; their localization keys are unrelated.
                if valid.iter().any(|(file, _)| *file == path)
                    && !report.msg.contains("localization")
                {
                    observed.push((path, loc.line, report.msg.clone()));
                }
            }
        }
        let expected = if malformed { expected.as_slice() } else { &[] };
        assert_eq!(observed.len(), expected.len(), "{observed:?}");
        for &(path, line, fragment) in expected {
            assert!(
                observed
                    .iter()
                    .any(|(p, n, msg)| p == path && *n == line && msg.contains(fragment)),
                "missing {path}:{line} {fragment} in {observed:?}"
            );
        }
    }
}
