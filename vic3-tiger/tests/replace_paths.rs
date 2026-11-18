use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn event(namespace: &str) -> String {
    format!(
        "\u{feff}namespace = {namespace}\n{namespace}.1 = {{ type = country_event hidden = yes }}\n"
    )
}

#[test]
fn replace_paths_hides_only_earlier_files_in_explicit_directories() {
    let temp = tempfile::tempdir().unwrap();
    let game = temp.path().join("install");
    let mod_dir = temp.path().join("mod");
    for dir in ["jomini", "clausewitz"] {
        fs::create_dir_all(game.join(dir)).unwrap();
    }
    write(&game, "launcher/launcher-settings.json", r#"{"rawVersion":"1.15.1"}"#);
    write(&game, "game/events/titanic_events.txt", &event("base_root"));
    write(&game, "game/events/law_events/laws.txt", &event("base_nested"));
    write(&mod_dir, "events/law_events/other.txt", &event("from_mod"));
    let checks = "common/on_actions/references.txt";
    write(
        &mod_dir,
        checks,
        "\u{feff}check_refs = {\n events = {\n base_root.1\n base_nested.1\n from_mod.1\n always_absent.1\n }\n}\n",
    );
    for (paths, expected) in [
        (vec![], vec![6]),
        (vec!["events"], vec![3, 6]),
        (vec!["events", "events/law_events"], vec![3, 4, 6]),
        (vec!["events/law_events"], vec![4, 6]),
        (vec![], vec![6]),
    ] {
        let metadata =
            json!({"name": "Replacement test", "game_custom_data": {"replace_paths": paths}});
        write(&mod_dir, ".metadata/metadata.json", &metadata.to_string());
        let result = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"))
            .arg("--json")
            .arg("--game")
            .arg(&game)
            .arg(&mod_dir)
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let reports: Vec<Value> = serde_json::from_slice(&result.stdout).unwrap();
        let mut lines = Vec::new();
        for report in reports {
            let loc = &report["locations"][0];
            if report["key"] == "missing-item"
                && loc["from"] == "MOD"
                && loc["path"].as_str().unwrap().replace('\\', "/") == checks
            {
                lines.push(loc["linenr"].as_u64().unwrap());
            }
        }
        lines.sort_unstable();
        assert_eq!(lines, expected, "replace_paths={paths:?}");
    }
}
