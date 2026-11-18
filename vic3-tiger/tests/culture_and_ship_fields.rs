use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn culture_and_ship_fields_are_valid_only_in_their_own_schema() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let culture = "common/cultures/test.txt";
    let names = "common/ship_name_definitions/test.txt";
    let ships = "common/ship_types/test.txt";
    write(root, "gfx/test.dds", "");
    write(root, "common/ai_ship_categories/test.txt", "category_test = {}\n");
    write(
        root,
        culture,
        "\u{feff}test_culture = {\n seal_and_signature_texture = gfx/test.dds\n seal_and_signature_textur = gfx/test.dds\n}\n",
    );
    write(
        root,
        names,
        "\u{feff}test_names = {\n properties = { {\n is_prefix = yes\n is_prefi = yes\n } }\n is_prefix = yes\n}\n",
    );
    write(
        root,
        ships,
        "\u{feff}test_ship = {\n ai_ship_category = category_test\n vulnerability_exploitation = 65\n ai_ship_categor = category_test\n vulnerability_exploitatio = 65\n}\nbad_ship = {\n ai_ship_category = category_missing\n vulnerability_exploitation = invalid\n}\n",
    );
    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut observed = Vec::new();
    for (report, locations) in take_reports() {
        for pointers in locations {
            let loc = &pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            // Ignore the synthetic objects' unrelated implied localization/modifier keys.
            if [culture, names, ships].contains(&path.as_str())
                && loc.line > 1
                && !(path == ships && loc.line == 7)
            {
                observed.push((path, loc.line, report.msg.clone()));
            }
        }
    }
    let expected = [
        (culture, 3, "seal_and_signature_textur"),
        (names, 4, "is_prefi"),
        (names, 6, "is_prefix"),
        (ships, 4, "ai_ship_categor"),
        (ships, 5, "vulnerability_exploitatio"),
        (ships, 8, "category_missing"),
        (ships, 9, "number"),
    ];
    assert_eq!(observed.len(), expected.len(), "{observed:?}");
    for (path, line, fragment) in expected {
        assert!(
            observed.iter().any(|(p, n, msg)| p == path && *n == line && msg.contains(fragment)),
            "missing {path}:{line} {fragment} in {observed:?}"
        );
    }
}
