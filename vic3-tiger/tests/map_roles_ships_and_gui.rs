use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{text}")).unwrap();
}

#[test]
fn validates_vic3_115_map_roles_ship_modifiers_and_rootless_gui() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let map = "gfx/map/map_modes/test.txt";
    let roles = "common/character_roles/test.txt";
    let ships = "common/ship_types/test.txt";
    let gui = "gui/test.gui";
    write(
        root,
        map,
        r#"mm_state = {
    map_painting_mode = state_infamy
    map_painting_mode_secondary = state_value
}
mm_journal = {
    map_painting_mode = journal_entry_map_data
    map_painting_mode_secondary = journal_entry_map_data_second
    map_names = none
}
mm_cold = { map_painting_mode = cold_effect }
mm_min_cold = { map_painting_mode = min_cold_effect }
mm_max_cold = { map_painting_mode = max_cold_effect }
mm_bad_secondary = { map_painting_mode = journal_entry_map_data_second }
mm_bad_names = { map_names = state_value }
mm_bad_paint = { map_painting_mode = typo }"#,
    );
    write(
        root,
        roles,
        r#"role_fixture = {
    type = other
    title_format = from_rank
}
role_bad_type = { type = from_rank }
role_bad_title = { title_format = other }"#,
    );
    write(
        root,
        ships,
        r#"fixture_ship = {
    modifier = {
        ship_power_projection_add = 1
        ship_power_projection_mult = 1
        ship_naval_invasion_efficiency_mult = 1
        ship_ship_type_aircraft_carrier_hull_damage_mult = 1
        ship_ship_type_submarine_hull_damage_mult = 1
        ship_ship_type_supply_ship_supply_capacity_mult = 1
        ship_ship_type_dreadnought_hull_damage_mult = 1
    }
}"#,
    );
    write(
        root,
        "common/scripted_guis/test.txt",
        r#"rootless = { scope = none is_valid = { always = yes } }
rootless_with_saved = {
    scope = none
    saved_scopes = { fixture_input }
    is_valid = { exists = scope:fixture_input }
}
rooted = { scope = country is_valid = { always = yes } }
rootless_without_effect = { scope = none }"#,
    );
    write(
        root,
        gui,
        r#"widget = {
    visible = "[GetScriptedGui('rootless').IsValid(GuiScope.End)]"
}
widget = {
    visible = "[GetScriptedGui('rootless_with_saved').IsValid(GuiScope.AddScope('fixture_input', GetPlayer.MakeScope).End)]"
}
widget = {
    visible = "[GetScriptedGui('rootless_with_saved').IsValid(GuiScope.End)]"
}
widget = {
    visible = "[GetScriptedGui('rooted').IsValid(GuiScope.End)]"
}
widget = {
    visible = "[GetScriptedGui('missing_sgui').IsValid(GuiScope.End)]"
}
widget = {
    onclick = "[GetScriptedGui('rootless_without_effect').Execute(GuiScope.End)]"
}
widget = {
    visible = "[GetScriptedGui('rootless').IsValid(GuiScope.Unrecognized.End)]"
}
widget = {
    visible = "[GetScriptedGui('rooted').IsValid(GuiScope.SetRoot(GetPlayer.MakeScope).End)]"
}"#,
    );

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let mut choices = Vec::new();
    let mut ship_kinds = Vec::new();
    let mut gui_reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = &pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            let key = report.key.to_string();
            if [map, roles].contains(&path.as_str()) && key == "choice" {
                choices.push((path, loc.line));
            } else if path == ships && report.msg == "modifier type definition does not exist" {
                ship_kinds.push((
                    loc.line,
                    report.info.as_deref().unwrap().contains("will apply the modifier"),
                ));
            } else if path == gui {
                gui_reports.push((loc.line, report.msg.clone()));
            }
        }
    }

    choices.sort_unstable();
    assert_eq!(
        choices,
        [
            (roles.to_owned(), 5),
            (roles.to_owned(), 6),
            (map.to_owned(), 13),
            (map.to_owned(), 14),
            (map.to_owned(), 15)
        ],
    );
    ship_kinds.sort_unstable();
    assert_eq!(
        ship_kinds,
        [(3, true), (4, true), (5, true), (6, true), (7, true), (8, true), (9, false)],
    );

    for (line, fragment) in [
        (8, "missing AddScope"),
        (11, "expected SetRoot"),
        (14, "missing_sgui"),
        (17, "has no effect block"),
        (20, "expected AddScope or End"),
    ] {
        assert!(
            gui_reports.iter().any(|(n, message)| *n == line && message.contains(fragment)),
            "missing gui diagnostic on line {line} for {fragment}: {gui_reports:?}",
        );
    }
    assert!(
        !gui_reports.iter().any(|(line, _)| [2, 5, 23].contains(line)),
        "unexpected report on valid gui: {gui_reports:?}",
    );
}
