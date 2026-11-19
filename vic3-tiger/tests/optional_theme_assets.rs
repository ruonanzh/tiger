use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, Severity, set_show_vanilla, take_reports};

const THEMES: &str = "common/themes/dlc_fixture.txt";
const VANILLA_THEMES: &str = "common/themes/dlc_base_fixture.txt";

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn checks_theme_assets_when_dlc_payload_is_available() {
    Game::set(Game::Vic3).unwrap();
    set_show_vanilla(true);
    let fixture = tempfile::tempdir().unwrap();
    let vanilla = fixture.path().join("vanilla");
    let game = vanilla.join("game");
    let mod_root = fixture.path().join("mod");
    for dir in ["clausewitz", "jomini"] {
        fs::create_dir_all(vanilla.join(dir)).unwrap();
    }

    write(
        &game,
        "dlc_metadata/fixture.txt",
        "dlc777 = { name = \"Example DLC\" theme_provider = yes }\n\
         dlc778 = { name = \"Not a theme provider\" theme_provider = no }\n",
    );
    write(
        &game,
        VANILLA_THEMES,
        "vanilla_theme = { category = ui_skin_theme dlc = dlc777 ui_skin = vanilla_missing }",
    );
    write(
        &mod_root,
        "dlc_metadata/mod_fixture.txt",
        "test_mod_dlc = { name = \"Unverified mod DLC\" theme_provider = yes }",
    );
    write(
        &mod_root,
        THEMES,
        concat!(
            "dlc_skin = { category = ui_skin_theme dlc = dlc777 ui_skin = provider_skin }\n",
            "dlc_objects = { category = table_top_theme dlc = dlc777\n",
            "    map_object = { pdxmesh = provider_mesh locator = example }\n",
            "    map_object = { entity = provider_entity locator = example }\n",
            "    map_object = { locator = example }\n",
            "    map_object = { pdxmesh = {} locator = example }\n",
            "    map_object = { pdxmesh = invalid:path locator = example }\n",
            "    video = gfx/missing.bk2\n",
            "    unknown_dlc_field = yes\n",
            "}\n",
            "ungated = { category = ui_skin_theme ui_skin = ungated_typo }\n",
            "invalid_gate = { category = ui_skin_theme dlc = unknown_pack ui_skin = unknown_typo }\n",
            "duplicate_gate = { category = ui_skin_theme dlc = dlc777 dlc = dlc777 ui_skin = duplicate_typo }\n",
            "mod_gate = { category = ui_skin_theme dlc = test_mod_dlc ui_skin = custom_typo }\n",
            "other_pack = { category = ui_skin_theme dlc = dlc778 ui_skin = other_pack_typo }\n",
            "existing_skin = { category = ui_skin_theme dlc = dlc777 ui_skin = recovered_skin }\n",
            "default_skin = { category = ui_skin_theme dlc = dlc777 ui_skin = default }\n",
            "main_menu_image_fixture = { category = main_menu_image_theme dlc = dlc777 video = dlc/dlc777_fixture/gfx/interface/illustrations/frontend/menu_video.bk2 }\n",
            "papermap_fixture = { category = papermap_theme dlc = dlc777 papermap_textures_file = gfx/map/textures/maptextures_fixture.settings }\n",
            "bad_pack = { category = main_menu_image_theme dlc = dlc777 video = dlc/dlc778_fixture/gfx/interface/illustrations/frontend/menu_video.bk2 }\n",
            "bad_path = { category = main_menu_image_theme dlc = dlc777 video = dlc/dlc777_fixture/../gfx/interface/illustrations/frontend/menu_video.bk2 }\n",
            "papermap_typo = { category = papermap_theme dlc = dlc777 papermap_textures_file = gfx/map/textures/maptextures_misspelled.settings }\n",
        ),
    );
    write(&mod_root, "gfx/skins/recovered.skin", "name = recovered_skin\nfolder = gfx/skins");

    // A differently named DLC directory must still match by its actual descriptor filename.
    let descriptor = "dlc/differently_named_package/dlc777.dlc";
    for installed in [false, true] {
        if installed {
            write(&game, descriptor, "name = \"Example DLC\"");
            write(
                &game,
                "dlc/differently_named_package/gfx/skins/provider.skin",
                "name = provider_skin\nfolder = gfx/skins",
            );
            write(
                &game,
                "dlc/differently_named_package/gfx/models/provider.asset",
                "entity = { name = provider_entity }",
            );
        }
        let mut data =
            Everything::new(None, Some(&vanilla), None, None, &mod_root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();
        let reports = take_reports();

        let mut note_lines = Vec::new();
        let mut missing_assets = Vec::new();
        let mut other_diagnostics = Vec::new();
        for (report, occurrences) in reports {
            for pointers in occurrences {
                let path = pointers[0].loc.pathname();
                if path != Path::new(THEMES) && path != Path::new(VANILLA_THEMES) {
                    continue;
                }
                let line = pointers[0].loc.line;
                match report.key.to_string().as_str() {
                    "optional-dlc" => {
                        assert_eq!(report.severity, Severity::Tips);
                        assert!(report.msg.contains("dlc777"), "{}", report.msg);
                        assert!(pointers.iter().all(|p| {
                            p.loc.pathname() == path && p.loc.column > 0 && p.msg.is_some()
                        }));
                        note_lines.push((
                            path.to_path_buf(),
                            pointers.iter().map(|p| p.loc.line).collect::<Vec<_>>(),
                        ));
                    }
                    "missing-item" => missing_assets.push((line, report.msg.clone())),
                    "field-missing" | "structure" | "missing-file" | "unknown-field" => {
                        other_diagnostics.push((line, report.msg.clone()));
                    }
                    _ => (),
                }
            }
        }

        let has_missing =
            |line, text| missing_assets.iter().any(|(nr, msg)| *nr == line && msg.contains(text));
        for (line, text) in [
            (7, "invalid:path"),
            (11, "ungated_typo"),
            (12, "unknown_pack"),
            (12, "unknown_typo"),
            (13, "duplicate_typo"),
            (14, "custom_typo"),
            (15, "other_pack_typo"),
        ] {
            assert!(has_missing(line, text), "{missing_assets:#?}");
        }
        for (line, text) in
            [(1, "provider_skin"), (4, "provider_entity"), (16, "recovered_skin"), (17, "default")]
        {
            assert!(!has_missing(line, text), "{missing_assets:#?}");
        }
        for (line, text) in [
            (5, "pdxmesh"),
            (6, "expected value"),
            (8, "missing.bk2"),
            (9, "unknown_dlc_field"),
            (20, "dlc778_fixture"),
            (21, "/../"),
            (22, "maptextures_misspelled.settings"),
        ] {
            assert!(
                other_diagnostics.iter().any(|(nr, msg)| *nr == line && msg.contains(text)),
                "{other_diagnostics:#?}"
            );
        }
        if installed {
            assert!(note_lines.is_empty(), "{note_lines:#?}");
            assert!(has_missing(1, "vanilla_missing"), "{missing_assets:#?}");
            assert!(has_missing(3, "provider_mesh"), "{missing_assets:#?}");
            assert!(
                other_diagnostics
                    .iter()
                    .any(|(line, msg)| { *line == 18 && msg.contains("menu_video.bk2") })
            );
            assert!(other_diagnostics.iter().any(|(line, msg)| {
                *line == 19 && msg.contains("maptextures_fixture.settings")
            }));
        } else {
            note_lines.sort_unstable();
            let mut expected =
                vec![(THEMES.into(), vec![1, 3, 4, 18, 19]), (VANILLA_THEMES.into(), vec![1])];
            expected.sort_unstable();
            assert_eq!(note_lines, expected);
            assert!(!has_missing(1, "vanilla_missing"), "{missing_assets:#?}");
            assert!(!has_missing(3, "provider_mesh"), "{missing_assets:#?}");
        }
    }
}
