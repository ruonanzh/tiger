use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

#[test]
fn validates_vic3_localization_expressions() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let relative_path = Path::new("localization/english/expression_probe_l_english.yml");
    let path = fixture.path().join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let contents = r##"l_english:
 PROBE_TEXT: "Text"
 PROBE_PARENS: "[ConcatIfNeitherEmpty(' (', ConcatIfNeitherEmpty('$PROBE_TEXT$', ')'))]"
 PROBE_DATES: "$[DATE_MIN.GetString|V]$ - $[DATE_MAX.GetString|V]$"
 PROBE_NESTED_MACRO: "$[ConcatIfNeitherEmpty('$PROBE_TEXT$', ')')]$"
 PROBE_MARKUP: "#[GetMoraleColorString(MilitaryFormation.GetMorale)] morale#!"
 PROBE_MARKUP_NEWLINE: "#indent_newline:2\nNext line#!"
 BAD_LITERAL: "[ConcatIfNeitherEmpty('Text', ')]"
 BAD_FUNCTION: "$[DATE_MIN.GetStrng|V]$"
 BAD_WRAPPER: "$[DATE_MIN.GetString|V]"
 BAD_NESTED_KEY: "$[ConcatIfNeitherEmpty('$MISSING_PROBE_KEY$', ')')]$"
 BAD_MARKUP_FUNCTION: "#[GetMoraleColorString()] morale#!"
 BAD_MARKUP_SEPARATOR: "#indent_newline:2\qNext line#!"
 PROBE_RECOVERY: "$[DATE_MIN.GetString|V]$"
"##;
    fs::write(path, format!("\u{feff}{contents}")).unwrap();

    let mut data = Everything::new(None, None, None, None, fixture.path(), Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let mut observed = Vec::new();
    for (report, occurrences) in &reports {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            if loc.pathname() == relative_path {
                observed.push((loc.line, report.msg.as_str()));
            }
        }
    }

    assert!(
        observed.iter().all(|(line, _)| (8..=13).contains(line)),
        "valid entries produced diagnostics: {observed:?}"
    );
    for expected in [
        (8, "unterminated argument string"),
        (9, "unknown datafunction GetStrng"),
        (10, "didn't recognize a key between $"),
        (11, "The substitution parameter $MISSING_PROBE_KEY$ is not defined anywhere as a key."),
        (12, "GetMoraleColorString takes 1 arguments but was given 0 here"),
        (13, "#markup should be followed by a space"),
    ] {
        assert!(observed.contains(&expected), "missing {expected:?} in {observed:?}");
    }
}
