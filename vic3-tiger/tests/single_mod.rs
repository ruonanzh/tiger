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
fn victoria_3_uses_built_in_settings_unless_config_is_explicit() {
    let fixture = tempfile::tempdir().unwrap();
    let game = fixture.path().join("game_install");
    let mod_dir = fixture.path().join("target_mod");
    let workshop = fixture.path().join("workshop");
    let workshop_mod = workshop.join("4100000000");
    let local_mod = fixture.path().join("local_addon");
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
        "\u{feff}#tiger-ignore(file)\nown_rule = { always = yes }\nprobe = { base_rule = yes own_rule = yes \
         workshop_rule = yes local_rule = yes filtered_rule = yes }",
    );
    write(
        &mod_dir,
        "localization/french/bad_l_french.yml",
        "\u{feff}l_french:\n #tiger-ignore(file)\n probe:0 without quotes\n",
    );

    let validation_command = |cwd: &Path| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vic3-tiger"));
        command.current_dir(cwd);
        command.args(["--json", "--game"]).arg(&game).arg("--workshop").arg(&workshop);
        command.arg(&mod_dir);
        command
    };
    let validate = |cwd: &Path, args: &[&str]| {
        let result = validation_command(cwd).args(args).output().unwrap();
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

    let expected = &["filtered_rule", "local_rule", "workshop_rule"];
    let unconfigured = validate(fixture.path(), &[]);
    assert_rules(&unconfigured, expected);
    assert!(reports_french(&unconfigured), "{unconfigured:#?}");

    let config_contents = "\u{feff}load_mod = { workshop_id = 4100000000 }\nload_mod = { mod = \"local_addon\" }\n\
         languages = { check = \"english\" }\nfilter = { trigger = { NOT = { text = \"filtered_rule\" } } }\n";
    write(&mod_dir, "vic3-tiger.conf", config_contents);
    write(fixture.path(), "vic3-tiger.conf", config_contents);
    let mod_subdirectory = mod_dir.join("common");
    write(&mod_subdirectory, "vic3-tiger.conf", config_contents);
    for cwd in [fixture.path(), &mod_dir, &mod_subdirectory] {
        for args in [&[][..], &["--single-mod"][..], &["--single-mod", "--no-config"][..]] {
            let reports = validate(cwd, args);
            assert_rules(&reports, expected);
            assert!(reports_french(&reports), "{reports:#?}");
        }
    }

    write(fixture.path(), "maintainer.conf", config_contents);
    let with_dependencies = validate(fixture.path(), &["--config", "maintainer.conf"]);
    assert_rules(&with_dependencies, &[]);
    assert!(!reports_french(&with_dependencies), "{with_dependencies:#?}");

    write(&mod_dir, "suppress.json", "[]");
    for mode in ["--single-mod", "--no-config"] {
        for (flag, file) in [("--config", "vic3-tiger.conf"), ("--suppress", "suppress.json")] {
            let result = validation_command(&mod_dir).args([mode, flag, file]).output().unwrap();
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert_eq!(result.status.code(), Some(2), "{stderr}");
            assert!(stderr.contains("cannot be used with") && stderr.contains(flag), "{stderr}");
        }
    }
}
