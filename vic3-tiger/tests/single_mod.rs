use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn single_mod_ignores_dependencies_and_preserves_config() {
    let fixture = tempfile::tempdir().unwrap();
    let game = fixture.path().join("game_install");
    let mod_dir = fixture.path().join("target_mod");
    let workshop = fixture.path().join("workshop");
    let workshop_mod = workshop.join("4100000000");
    let local_mod = mod_dir.join("local_addon");
    let triggers = "common/scripted_triggers/target.txt";

    for dir in ["jomini", "clausewitz"] {
        fs::create_dir_all(game.join(dir)).unwrap();
    }
    write(&game, "launcher/launcher-settings.json", r#"{"rawVersion":"1.15.2"}"#);
    write(&game, "game/events/titanic_events.txt", "\u{feff}");
    write(&game, "game/common/scripted_triggers/base.txt", "\u{feff}base_rule = { always = yes }");
    for root in [&mod_dir, &workshop_mod, &local_mod] {
        write(root, ".metadata/metadata.json", r#"{"name":"Test mod"}"#);
    }
    write(
        &workshop_mod,
        "common/scripted_triggers/workshop.txt",
        "\u{feff}workshop_rule = { always = yes }",
    );
    write(
        &local_mod,
        "common/scripted_triggers/local.txt",
        "\u{feff}local_rule = { always = yes }",
    );
    write(
        &mod_dir,
        triggers,
        "\u{feff}own_rule = { always = yes }\nprobe = { base_rule = yes own_rule = yes \
         workshop_rule = yes local_rule = yes filtered_rule = yes }",
    );
    write(&mod_dir, "localization/french/bad_l_french.yml", "\u{feff}invalid header\n");

    let validate = |single_mod| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"));
        command.args(["--json", "--game"]).arg(&game).arg("--workshop").arg(&workshop);
        if single_mod {
            command.arg("--single-mod");
        }
        let result = command.arg(&mod_dir).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let reports: Vec<Value> = serde_json::from_slice(&result.stdout).unwrap();
        reports
    };
    let assert_rules = |reports: &[Value], expected: &[&str]| {
        let mut messages = Vec::new();
        for report in reports {
            if report["key"] == "unknown-field" {
                assert_eq!(report["severity"], "error", "{report}");
                messages.push(report["message"].as_str().unwrap());
            }
        }
        messages.sort_unstable();
        let expected: Vec<_> =
            expected.iter().map(|rule| format!("unknown token `{rule}`")).collect();
        assert_eq!(messages, expected, "{reports:#?}");
    };
    let reports_french = |reports: &[Value]| {
        reports.iter().any(|report| {
            report["locations"].as_array().unwrap().iter().any(|loc| {
                loc["path"].as_str().is_some_and(|path| path.ends_with("bad_l_french.yml"))
            })
        })
    };

    let unconfigured = validate(false);
    assert_rules(&unconfigured, &["filtered_rule", "local_rule", "workshop_rule"]);
    assert!(reports_french(&unconfigured), "{unconfigured:#?}");

    write(
        &mod_dir,
        "vic3-tiger.conf",
        "\u{feff}load_mod = { workshop_id = 4100000000 }\nload_mod = { mod = \"local_addon\" }\n\
         languages = { check = \"english\" }\nfilter = { trigger = { NOT = { text = \"filtered_rule\" } } }\n",
    );
    let with_dependencies = validate(false);
    assert_rules(&with_dependencies, &[]);
    assert!(!reports_french(&with_dependencies), "{with_dependencies:#?}");

    let isolated = validate(true);
    assert_rules(&isolated, &["local_rule", "workshop_rule"]);
    assert!(!reports_french(&isolated), "{isolated:#?}");
}
