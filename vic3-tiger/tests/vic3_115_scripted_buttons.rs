use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

const BUTTONS: &str = "common/scripted_buttons/fixture.txt";
const GROUPS: &str = "common/journal_entry_groups/fixture.txt";
const VALID_BUTTONS: &str = r#"button_fixture = {
    name = button_title
    short_desc = button_summary
    icon = "gfx/button_fixture.dds"
    cost = button_cost
    autocomplete = yes
    tags = { unlocked_by_ruler arbitrary_mod_tag }
}
button_inline_fixture = {
    name = button_title
    cost = {
        value = 100
        if = {
            limit = { exists = scope:journal_entry exists = scope:target }
            add = bureaucracy
        }
    }
    tags = {}
}
button_numeric_fixture = { name = button_title cost = 100 }
"#;

const INVALID_BUTTONS: &str = r#"button_fixture = {
    name = button_title
    short_desc = {}
    icon = "gfx/missing_button.dds"
    cost = missing_button_cost
    autocomplete = maybe
    tags = { unlocked_by_ruler invalid = value }
}
button_inline_fixture = {
    name = button_title
    short_desc = missing_button_summary
    cost = { value = scope:journal_entry }
    tags = arbitrary_mod_tag
}
button_numeric_fixture = {
    name = button_title
    autocomplet = yes
    icon = {}
}
"#;

fn write_fixture(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_scripted_button_and_themed_journal_group_fields() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_fixture(root, "gfx/button_fixture.dds", "");
    write_fixture(root, "common/script_values/fixture.txt", "button_cost = 25\n");
    write_fixture(
        root,
        "localization/english/fixture_l_english.yml",
        "l_english:\n button_title: \"Button\"\n button_summary: \"Summary\"\n group_fixture: \"Group\"\n",
    );
    let expected = [
        (BUTTONS, 3, "expected value, found block"),
        (BUTTONS, 4, "missing_button.dds"),
        (BUTTONS, 5, "missing_button_cost"),
        (BUTTONS, 6, "expected yes or no"),
        (BUTTONS, 7, "expected"),
        (BUTTONS, 11, "missing_button_summary"),
        (BUTTONS, 12, "journal"),
        (BUTTONS, 13, "expected block"),
        (BUTTONS, 17, "unknown field `autocomplet`"),
        (BUTTONS, 18, "expected value, found block"),
        (GROUPS, 2, "expected yes or no"),
        (GROUPS, 3, "unknown field `theme`"),
    ];

    for malformed in [false, true, false] {
        write_fixture(root, BUTTONS, if malformed { INVALID_BUTTONS } else { VALID_BUTTONS });
        let groups = if malformed {
            "group_fixture = {\n    themed = 1\n    theme = yes\n}\n"
        } else {
            "group_fixture = {\n    themed = yes\n}\n"
        };
        write_fixture(root, GROUPS, groups);

        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut observed = Vec::new();
        for (report, locations) in take_reports() {
            for pointers in locations {
                let loc = &pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                if path == BUTTONS || path == GROUPS {
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
