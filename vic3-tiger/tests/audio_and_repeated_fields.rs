use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

const ALIASES: &str = "gfx/media_aliases/catalog_fixture.txt";
const TEMPLATES: &str = "common/character_templates/catalog_fixture.txt";
const EVENTS: &str = "events/catalog_fixture.txt";

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{text}")).unwrap();
}

#[test]
fn checks_audio_catalog_and_repeated_character_roles_and_ships() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let vanilla = fixture.path().join("vanilla");
    let mod_root = fixture.path().join("mod");
    for directory in ["clausewitz", "jomini"] {
        fs::create_dir_all(vanilla.join(directory)).unwrap();
    }
    write(
        &vanilla,
        "game/sound/GUIDs.txt",
        concat!(
            "{c54e0270-18d8-4902-a228-19dbb989adf8} bank:/Fixture\n",
            "{de7d4bd5-bc29-44a0-b33e-7f25f46116c6} event:/Fixture/Surface Bell\n",
            "{02b2a119-ac50-432d-bb80-36f0b1ac9b19} snapshot:/Fixture/Quiet\n",
        ),
    );
    write(
        &mod_root,
        "common/character_roles/catalog_fixture.txt",
        "fixture_primary = {}\nfixture_secondary = {}",
    );
    write(
        &mod_root,
        "common/ship_types/catalog_fixture.txt",
        "fixture_frigate = {}\nfixture_cruiser = {}",
    );

    for broken in [false, true, false] {
        let audio = if broken { "event:/Fixture/Typo" } else { "EVENT:/fixture/surface bell" };
        let role = if broken { "fixture_role_typo" } else { "fixture_secondary" };
        let ship = if broken { "fixture_ship_typo" } else { "fixture_cruiser" };
        write(
            &mod_root,
            ALIASES,
            &format!(
                "fixture_event = {{ audio = \"{audio}\" }}\n\
                 fixture_snapshot = {{ audio = \"snapshot:/Fixture/Quiet\" }}\n\
                 fixture_silent = {{ audio = \"\" }}"
            ),
        );
        write(
            &mod_root,
            TEMPLATES,
            &format!("fixture_character = {{\n  role = fixture_primary\n  role = {role}\n}}"),
        );
        write(
            &mod_root,
            EVENTS,
            &format!(
                "namespace = catalog_fixture
                catalog_fixture.1 = {{
                    type = country_event
                    hidden = yes
                    immediate = {{
                        create_character = {{
                            role = fixture_primary
                            role = {role}
                        }}
                        create_military_formation = {{
                            type = fleet
                            name = fixture_fleet
                            ship = {{ type = ship_type:fixture_frigate count = 1 }}
                            ship = {{ type = ship_type:{ship} count = 1 }}
                        }}
                    }}
                }}"
            ),
        );

        let mut data =
            Everything::new(None, Some(&vanilla), None, None, &mod_root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut errors = Vec::new();
        for (report, occurrences) in take_reports() {
            let key = report.key.to_string();
            if !matches!(key.as_str(), "missing-item" | "duplicate-field" | "structure") {
                continue;
            }
            for pointers in occurrences {
                let loc = &pointers[0].loc;
                for path in [ALIASES, TEMPLATES, EVENTS] {
                    if loc.pathname() == Path::new(path) {
                        errors.push((path, loc.line, key.clone(), report.msg.clone()));
                    }
                }
            }
        }

        if broken {
            assert_eq!(errors.len(), 4, "{errors:#?}");
            for (path, line, needle) in [
                (ALIASES, 1, "unknown sound event:/Fixture/Typo"),
                (TEMPLATES, 3, "character role fixture_role_typo"),
                (EVENTS, 8, "character role fixture_role_typo"),
                (EVENTS, 14, "ship type fixture_ship_typo"),
            ] {
                assert!(
                    errors.iter().any(|(p, nr, key, msg)| {
                        *p == path && *nr == line && key == "missing-item" && msg.contains(needle)
                    }),
                    "missing {path}:{line} {needle:?}\n{errors:#?}"
                );
            }
        } else {
            assert!(errors.is_empty(), "{errors:#?}");
        }
    }
}
