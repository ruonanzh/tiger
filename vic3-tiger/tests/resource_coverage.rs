use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, Severity, take_reports};

fn write(root: &Path, relative: &str, contents: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn reports_coverage_as_tips_and_retains_proven_format_diagnostics() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(
        root,
        "common/production_methods/fixture.txt",
        "\u{feff}pm_fixture = { is_default = yes texture = gfx/ui/mixed.dds }",
    );
    write(
        root,
        "gfx/map/city_data/city_types/fixture.txt",
        "\u{feff}fixture_city = { city_type = city rich_building_meshes = { absent_city_mesh } }",
    );
    write(
        root,
        "gfx/map/city_data/city_centerpiece/fixture.txt",
        "\u{feff}fixture_center = { city_type = city attach = { variant = { entity = absent_city_entity } } }",
    );
    write(
        root,
        "localization/english/first_l_english.yml",
        "\u{feff}l_english:\n same: \"same value\"\n differing: \"first value\"\n differing: \"second value\"\n across: \"first file\"\n shadowed: \"global winner\"\n overridden: \"base value\"\n pm_fixture: 0 \"Fixture\"\n",
    );
    write(
        root,
        "localization/english/second_l_english.yml",
        "\u{feff}l_english:\n same: \"same value\"\n across: \"second file\"\n shadowed: \"first local\"\n shadowed: \"second local\"\n recovered:\n recovered: \"first valid\"\n recovered: \"second valid\"\n",
    );
    write(
        root,
        "localization/replace/english/fixture_l_english.yml",
        "\u{feff}l_english:\n overridden: \"first replacement\"\n overridden: \"second replacement\"\n",
    );
    write(
        root,
        "localization/english/quote_l_english.yml",
        format!(
            "\u{feff}{}",
            r#"l_english:
 ambiguous: "first "unescaped" value"
 ambiguous: "second value"
 escaped: "first \"escaped\" value"
 escaped: "second value" # safe comment
 trailing: "first value" invalid suffix
 trailing: "second value"
 trailing: "third value"
"#,
        ),
    );
    write(
        root,
        "localization/french/first_l_french.yml",
        "\u{feff}\u{feff}l_german:\n other: \"Fixture\"\n other: \"Different fixture\"\n",
    );
    write(
        root,
        "common/script_values/braces.txt",
        "\u{feff}balanced = {\n if = { limit = { always = yes } add = 1\n}\n}\nunclosed = {\n",
    );
    write(root, "gfx/bad.dds", [0; 128]);
    let mut valid_header = [0; 128];
    valid_header[..4].copy_from_slice(b"DDS ");
    write(root, "gfx/ui/Mixed.dds", valid_header);

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    let reports = take_reports();
    let find = |key: &str, message: &str| {
        reports
            .iter()
            .flat_map(|(report, occurrences)| occurrences.iter().map(move |locs| (report, locs)))
            .filter(|(report, _)| report.key.to_string() == key && report.msg.contains(message))
            .collect::<Vec<_>>()
    };
    for (key, message, severity) in [
        ("missing-localization", "french localization key pm_fixture", Severity::Tips),
        ("missing-item", "pdxmesh absent_city_mesh", Severity::Tips),
        ("missing-item", "entity absent_city_entity", Severity::Tips),
        ("missing-file", "gfx/ui/mixed.dds", Severity::Tips),
        ("duplicate-item", "localization key `differing`", Severity::Warning),
        ("duplicate-item", "localization key `escaped`", Severity::Warning),
        ("duplicate-item", "localization key `ambiguous`", Severity::Tips),
        ("duplicate-item", "localization key `trailing`", Severity::Tips),
        ("duplicate-item", "localization key `trailing`", Severity::Warning),
        ("duplicate-item", "localization key `same`", Severity::Tips),
        ("duplicate-item", "localization key `across`", Severity::Tips),
        ("localization", "wrong language header", Severity::Warning),
        ("encoding", "double BOM", Severity::Warning),
        ("brace-placement", "possible brace error", Severity::Tips),
        ("brace-error", "", Severity::Error),
        ("image-format", "not a DDS file", Severity::Error),
    ] {
        let matches = find(key, message);
        assert!(
            matches.iter().any(|(report, _)| report.severity == severity),
            "{key} {message}: {matches:?}"
        );
    }
    for (key, path, local_values) in [
        ("shadowed", "localization/english/second_l_english.yml", ["first local", "second local"]),
        (
            "overridden",
            "localization/replace/english/fixture_l_english.yml",
            ["first replacement", "second replacement"],
        ),
    ] {
        let duplicates = find("duplicate-item", &format!("localization key `{key}`"));
        assert_eq!(duplicates.len(), 2, "{duplicates:?}");
        let (warning, pointers) =
            duplicates.iter().find(|(report, _)| report.severity == Severity::Warning).unwrap();
        assert_eq!(warning.severity, Severity::Warning);
        assert!(pointers.iter().all(|pointer| pointer.loc.pathname() == Path::new(path)));
        assert!(local_values.iter().all(|value| {
            pointers.iter().any(|p| p.msg.as_deref().is_some_and(|msg| msg.contains(value)))
        }));
        let (_, pointers) =
            duplicates.iter().find(|(report, _)| report.severity == Severity::Tips).unwrap();
        assert_ne!(pointers[0].loc.pathname(), pointers[1].loc.pathname());
    }
    let unconfirmed_language = find("duplicate-item", "localization key `other`");
    assert_eq!(unconfirmed_language.len(), 1);
    assert_eq!(unconfirmed_language[0].0.severity, Severity::Tips);
    let recovered = find("duplicate-item", "localization key `recovered`");
    assert_eq!(recovered.len(), 2);
    assert_eq!(
        recovered.iter().filter(|(report, _)| report.severity == Severity::Warning).count(),
        1
    );
    let capped = find("missing-file", "gfx/ui/mixed.dds");
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].0.info.as_deref(), Some("Existing path candidates: gfx/ui/Mixed.dds"));

    let cross_file = find("duplicate-item", "localization key `across`");
    assert_eq!(cross_file.len(), 1);
    let pointers = cross_file[0].1;
    assert_eq!(pointers.len(), 2);
    assert!(
        pointers.iter().any(|p| p.msg.as_deref().is_some_and(|msg| msg.contains("first file")))
    );
    assert!(
        pointers.iter().any(|p| p.msg.as_deref().is_some_and(|msg| msg.contains("second file")))
    );
}
