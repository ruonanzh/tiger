use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, LogReportMetadata, Severity, take_reports};

const TEXT: &str = "localization/english/text_probe_l_english.yml";
const GUI: &str = "gui/applicationutils/editor_probe.gui";
const SCRIPT: &str = "common/script_values/reader_probe.txt";
const GENES: &str = "common/genes/aging_probe.txt";
const ASSET: &str = "gfx/models/portraits/attachments/probe.asset";

fn check(root: &Path, contents: [(&str, &str); 5]) -> Vec<(String, u32, LogReportMetadata)> {
    for (path, text) in contents {
        let destination = root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        if path == TEXT || path == SCRIPT {
            fs::write(destination, format!("\u{feff}{text}")).unwrap();
        } else {
            fs::write(destination, text).unwrap();
        }
    }
    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    take_reports()
        .into_iter()
        .flat_map(|(report, occurrences)| {
            occurrences.into_iter().filter_map(move |pointers| {
                let loc = pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                [TEXT, GUI, SCRIPT, GENES, ASSET]
                    .contains(&path.as_str())
                    .then(|| (path, loc.line, report.clone()))
            })
        })
        .collect()
}

#[test]
fn vic3_text_and_portrait_diagnostics_distinguish_valid_and_invalid_forms() {
    Game::set(Game::Vic3).unwrap();
    let root = tempfile::tempdir().unwrap();
    let concepts = root.path().join("common/game_concepts");
    fs::create_dir_all(&concepts).unwrap();
    fs::write(concepts.join("probe.txt"), "\u{feff}registered_probe = {}").unwrap();
    let good = [
        (
            TEXT,
            r##"l_english:
 WORD: "money"
 12: "twelve"
 PREFIX: "Paid US$1,240 ($248/m3) or 35,50 $, not $WORD$."
 SUFFIX: "Pay 39,50 $ per barrel (248 $/m3)."
 NUMERIC_KEY: "$12$"
 MARKUP: "#Sreally#! and #r10#!"
 CONCEPT: "[Concept('registered_probe', 'Label')]"
"##,
        ),
        (GUI, "@width = 200\ntemplate probe { widget = { size = { @width 40 } } }"),
        (SCRIPT, "@width = 200\namount = @width"),
        (
            GENES,
            "\u{feff}age_presets = { probe = { mode = multiply curve = { { -1 -1 } { 1 0 } } } }",
        ),
        (
            ASSET,
            "entity = { name = probe game_data = { portrait_entity_user_data = { color_mask_remap_interval = { interval = { 1 0 } } } } }",
        ),
    ];
    let reports = check(root.path(), good);
    assert!(
        reports.iter().all(|(path, line, report)| {
            path == TEXT
                && *line == 7
                && report.msg == "#markup should be followed by a space"
                && report.severity == Severity::Tips
                && report.confidence == Confidence::Weak
        }),
        "{reports:?}"
    );
    assert!(!reports.is_empty(), "missing markup layout advice");

    let bad = [
        (
            TEXT,
            r##"l_english:
 INVALID_KEY: "$broken key$"
 UNTERMINATED: "$broken"
 INVALID_NUMBER_KEY: "$12 broken$"
 INVALID_BINDING: "[ConcatIfNeitherEmpty('$bad key$', 'suffix')]"
 INVALID_BRACE: "#color:{0.1,1"
 RECOVERY: "Paid $20"
 LATER_INVALID: "Paid $248/m3, not $12 broken$"
 CONCEPT: "[Concept(' registered_probe', 'Label')]"
 UNKNOWN_CONCEPT: "[Concept(' missing_probe', 'Label')]"
"##,
        ),
        (GUI, "template probe { widget = { size = { @runtime_width 40 } addcolumn = bad } }"),
        (SCRIPT, "amount = @missing"),
        (
            GENES,
            "\u{feff}age_presets = { probe = { mode = multiply curve = { { -1 0 } { 0 1.1 } { 1 nope } } } }",
        ),
        (
            ASSET,
            "entity = { name = probe game_data = { portrait_entity_user_data = { color_mask_remap_interval = { interval = { 2 nope } } } } }",
        ),
    ];
    let reports = check(root.path(), bad);
    let has = |path, line, severity, msg| {
        reports.iter().any(|(p, l, r)| {
            p == path && (line == 0 || *l == line) && r.severity == severity && r.msg == msg
        })
    };
    for line in 2..=5 {
        assert!(
            has(TEXT, line, Severity::Warning, "didn't recognize a key between $"),
            "{reports:?}"
        );
    }
    for (path, line, severity, msg) in [
        (TEXT, 6, Severity::Warning, "mismatched braces in markup"),
        (GUI, 1, Severity::Tips, "reader variable runtime_width not defined"),
        (GUI, 1, Severity::Error, "expected number"),
        (SCRIPT, 1, Severity::Error, "reader variable missing not defined"),
        (GENES, 1, Severity::Error, "expected number from 0.0 to 1.0"),
        (GENES, 1, Severity::Error, "expected number from -1.0 to 1.0"),
        (GENES, 1, Severity::Error, "expected number"),
        (ASSET, 1, Severity::Warning, "expected number between 0 and 1"),
        (ASSET, 1, Severity::Error, "expected number"),
    ] {
        assert!(has(path, line, severity, msg), "missing {path}:{line} {msg}: {reports:?}");
    }
    assert_eq!(
        reports
            .iter()
            .filter(|(path, _, report)| path == GUI && report.msg == "expected number")
            .count(),
        1,
        "{reports:?}"
    );
    assert!(!reports.iter().any(|(path, line, _)| path == TEXT && *line == 7), "{reports:?}");
    assert!(has(TEXT, 8, Severity::Warning, "didn't recognize a key between $"), "{reports:?}");
    assert_eq!(reports.iter().filter(|(path, line, _)| path == TEXT && *line == 8).count(), 1);
    assert!(has(
        TEXT,
        9,
        Severity::Tips,
        "Concept argument has whitespace around registered key `registered_probe`"
    ));
    assert!(has(
        TEXT,
        10,
        Severity::Warning,
        "game concept  missing_probe not defined in common/game_concepts/"
    ));

    let restored = check(root.path(), good);
    assert!(restored.iter().all(|(path, line, _)| path == TEXT && *line == 7), "{restored:?}");
}
