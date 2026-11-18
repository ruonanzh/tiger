use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

fn write_fixture(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_vic3_115_event_and_production_method_rules() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let events = "events/schema.txt";
    let methods = "common/production_methods/schema.txt";

    write_fixture(root, "common/regional_exile_pools/schema.txt", "pool_test = {}\n");
    write_fixture(root, "map_data/state_regions/schema.txt", "STATE_TEST = {}\n");
    write_fixture(root, "common/buildings/schema.txt", "building_test = {}\n");
    write_fixture(
        root,
        events,
        concat!(
            "namespace = schema\n",
            "schema.1 = {\n",
            "    type = country_event\n",
            "    hidden = yes\n",
            "    fire_only_once = yes\n",
            "    trigger = {\n",
            "        has_building_levels_in_state_region = { region = STATE_TEST building = building_test levels >= 2 }\n",
            "        has_building_levels_in_state_region = { region = STATE_TEST building = building_test levels = 2 is_incorporated = no }\n",
            "        has_building_levels_in_state_region = { region = STATE_TEST building = building_test levels = 2 is_incorporate = no }\n",
            "        any_scope_war = { has_war_support_change = { target = root value < 0 } }\n",
            "    }\n",
            "    immediate = { renege_treaty_ports_with = root }\n",
            "    option = { fire_only_once = yes }\n",
            "}\n",
            "schema.2 = {\n",
            "    type = character_event\n",
            "    hidden = yes\n",
            "    trigger = {\n",
            "        is_in_regional_exile_pool = no\n",
            "        is_in_regional_exile_pool = { exile_pool = pool_test }\n",
            "        is_in_regional_exile_pool = { exile_pool = pool_test value = no }\n",
            "        is_in_regional_exile_pool = { exile_pool = pool_missing }\n",
            "    }\n",
            "}\n",
        ),
    );
    write_fixture(
        root,
        methods,
        concat!(
            "pm_test = {\n",
            "    scoring_override = low_pop_method\n",
            "    scoring_overrid = low_pop_method\n",
            "}\n",
            "pm_bad = {\n",
            "    scoring_override = {}\n",
            "}\n",
        ),
    );

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut observed = Vec::new();
    for (report, locations) in take_reports() {
        for pointers in locations {
            let loc = &pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            // The synthetic production methods have no localization.
            if path == events || (path == methods && ![1, 5].contains(&loc.line)) {
                observed.push((path, loc.line, report.msg.clone()));
            }
        }
    }
    let expected = [
        (events, 9, "is_incorporate"),
        (events, 13, "fire_only_once"),
        (events, 22, "pool_missing"),
        (methods, 3, "scoring_overrid"),
        (methods, 6, "expected value, found block"),
    ];
    assert_eq!(observed.len(), expected.len(), "{observed:?}");
    for (path, line, fragment) in expected {
        assert!(
            observed.iter().any(|(p, n, msg)| p == path && *n == line && msg.contains(fragment)),
            "missing {path}:{line} {fragment} in {observed:?}"
        );
    }
}
