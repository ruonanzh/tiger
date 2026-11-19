use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

const AIR: &str = "gfx/map/air_graphics/migration_test.txt";
const FLEET: &str = "gfx/map/fleet_entities/wind_test.txt";
const MAP: &str = "gfx/map/map_modes/supply_test.txt";

const FIXTURES: [(&str, &str, &str); 3] = [
    (
        AIR,
        "migration_test = {
    breeding_latitude = { 3400.123456 -2600 }
    wintering_latitude = { 900 2044.123456 }
    breeding_regions = { region_test }
    wintering_regions = {}
    depart_in = { spring mod_season }
    return_in = { autumn }
}
independent_band = { wintering_latitude = { 0 0 } }
",
        "migration_test = {
    breeding_latitud = { 2600 3400 }
    wintering_latitud = { 900 2044 }
    breeding_region = { region_test }
    wintering_region = { region_test }
    depart_i = { spring }
    return_i = { autumn }
}
bad_migration = {
    breeding_latitude = { 2600 }
    wintering_latitude = { 900 word 2044 }
    breeding_regions = { region_missing }
    wintering_regions = region_test
    depart_in = spring
    return_in = { { autumn } }
}
bad_band = { breeding_latitude = { 2600 stray = 1 3400 } }
",
    ),
    (
        FLEET,
        "ship_test = {
    max_wind_lean = 10.123456
    flagship_signalflags_entity = test_flag_entity
}
",
        "ship_test = {
    max_wind_lea = 10
    flagship_signalflags_entit = test_flag_entity
    max_wind_lean = windy
    flagship_signalflags_entity = missing_flag_entity
}
",
    ),
    (
        MAP,
        "supply_test = {
    show_supply_route_arrows = yes
}
hidden_supply_test = { show_supply_route_arrows = no }
",
        "supply_test = {
    show_supply_route_arrow = yes
    show_supply_route_arrows = 1
}
",
    ),
];

fn write_fixture(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_air_migration_fleet_graphics_and_supply_route_arrows() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_fixture(root, "common/strategic_regions/test.txt", "region_test = {}");
    write_fixture(root, "common/ship_types/test.txt", "ship_test = {}");
    write_fixture(root, "gfx/models/test.asset", "entity = { name = test_flag_entity }");
    write_fixture(
        root,
        "localization/english/fixture_l_english.yml",
        "l_english:\n supply_test: \"Supply\"\n hidden_supply_test: \"Hidden supply\"\n",
    );

    let expected = [
        (AIR, 2, "unknown field `breeding_latitud`"),
        (AIR, 3, "unknown field `wintering_latitud`"),
        (AIR, 4, "unknown field `breeding_region`"),
        (AIR, 5, "unknown field `wintering_region`"),
        (AIR, 6, "unknown field `depart_i`"),
        (AIR, 7, "unknown field `return_i`"),
        (AIR, 10, "expected 2 numbers"),
        (AIR, 11, "expected number"),
        (AIR, 12, "region_missing"),
        (AIR, 13, "expected block"),
        (AIR, 14, "expected block"),
        (AIR, 15, "expected value, found block"),
        (AIR, 17, "unknown field `stray`"),
        (FLEET, 2, "unknown field `max_wind_lea`"),
        (FLEET, 3, "unknown field `flagship_signalflags_entit`"),
        (FLEET, 4, "expected number"),
        (FLEET, 5, "missing_flag_entity"),
        (MAP, 2, "unknown field `show_supply_route_arrow`"),
        (MAP, 3, "expected yes or no"),
    ];

    for malformed in [false, true, false] {
        for (path, valid, invalid) in FIXTURES {
            write_fixture(root, path, if malformed { invalid } else { valid });
        }

        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut observed = Vec::new();
        for (report, locations) in take_reports() {
            for pointers in locations {
                let loc = &pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                if FIXTURES.iter().any(|(file, _, _)| *file == path) {
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
