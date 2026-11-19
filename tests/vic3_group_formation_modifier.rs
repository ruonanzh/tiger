#![cfg(feature = "vic3")]

use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, LogReportMetadata, Severity, take_reports};

const ASSET: &str = "gfx/models/group_schema.asset";
const FORMATION: &str = "common/country_formation/group_schema.txt";
const MODIFIER: &str = "common/modifier_type_definitions/group_schema.txt";

fn write_fixture(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn validate(
    root: &Path,
    assets: &str,
    formations: &str,
    modifier: &str,
) -> Vec<(String, LogReportMetadata)> {
    write_fixture(root, ASSET, assets);
    write_fixture(root, FORMATION, formations);
    write_fixture(root, MODIFIER, modifier);
    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            // The definition headers can have unrelated country or localization diagnostics.
            if path == ASSET || ((path == FORMATION || path == MODIFIER) && loc.line > 1) {
                reports.push((path, report.clone()));
            }
        }
    }
    reports
}

#[test]
fn vic3_group_formation_and_hidden_schemas_validate_and_recover() {
    Game::set(Game::Vic3).unwrap();
    let root = std::env::temp_dir().join(format!("vic3-tiger-schema-{}", std::process::id()));
    let good_assets = r#"
entity = { name = scaffold_a }
entity = { name = scaffold_b }
entity = {
    name = building
    locator = { name = socket }
    group = {
        name = random_layout
        attach = { chance = 10 socket = scaffold_a socket = scaffold_b }
        attach = { chance = 2.5 socket = scaffold_b }
    }
    group = {
        attach = { chance = 1 socket = scaffold_a }
    }
}"#;
    let good_formation = "AAA = {\n should_target_state_in_unification_play = {}\n}";
    let good_modifier = "country_prestige_add = {\n hidden = yes\n}";
    let clean = validate(&root, good_assets, good_formation, good_modifier);
    assert!(clean.is_empty(), "{clean:?}");

    let broken_assets = r#"
entity = { name = scaffold_a }
entity = {
    name = building
    group = {
        name = {}
        atach = {}
        attach = unexpected
        attach = { chance = frequent socket = misspelled_scaffold }
        attach = { chance = 10 socket = {} }
    }
}"#;
    let broken_formation = "AAA = {\n should_target_state_in_unification_play = yes\n}\n\
                             BBB = {\n should_target_state_in_unification_play = { always = yes }\n}";
    let broken_modifier = "country_prestige_add = {\n hidden = sometimes\n}";
    let reports = validate(&root, broken_assets, broken_formation, broken_modifier);
    for (file, fragment) in [
        (ASSET, "expected value, found block"),
        (ASSET, "unknown field `atach`"),
        (ASSET, "expected block, found value"),
        (ASSET, "expected number"),
        (ASSET, "misspelled_scaffold"),
        (ASSET, "unknown field `socket`"),
        (FORMATION, "expected block, found value"),
        (MODIFIER, "expected yes or no"),
    ] {
        assert!(
            reports.iter().any(|(path, report)| path == file && report.msg.contains(fragment)),
            "missing {fragment:?} in {file}: {reports:?}"
        );
    }
    assert!(reports.iter().any(|(path, report)| {
        path == FORMATION
            && report.msg == "non-empty should_target_state_in_unification_play needs scope review"
            && report.severity == Severity::Tips
            && report.confidence == Confidence::Weak
    }));

    let recovered =
        validate(&root, good_assets, good_formation, "country_prestige_add = {\n hidden = no\n}");
    assert!(recovered.is_empty(), "{recovered:?}");
    fs::remove_dir_all(root).unwrap();
}
