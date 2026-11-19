use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

const TEMPLATE: &str = "common/character_templates/fixture.txt";
const EVENTS: &str = "events/fixture.txt";

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, format!("\u{feff}{text}")).unwrap();
}

#[test]
fn formation_supply_hubs_and_executive_templates_accept_evidenced_scopes() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(root, "map_data/state_regions/fixture.txt", "STATE_FIXTURE = { id = 1 provinces = {} }");
    write(root, "common/company_types/fixture.txt", "company_fixture = {}");

    for broken in [false, true, false] {
        let supply_hub = if broken { "root" } else { "s:STATE_FIXTURE" };
        let company = if broken { "root" } else { "company:company_fixture" };
        write(
            root,
            EVENTS,
            &format!(
                "namespace = fixture\nfixture.1 = {{ type = country_event hidden = yes immediate = {{\ncreate_military_formation = {{ type = army supply_hub = {supply_hub} }}\ncreate_military_formation = {{ type = army supply_hub = capital }}\n    }}\n}}"
            ),
        );
        write(
            root,
            TEMPLATE,
            &format!(
                "fixture_executive = {{\ncompany = {company}\n}}\nfixture_legacy = {{ company = company_type:company_fixture }}"
            ),
        );

        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let found: Vec<_> = take_reports()
            .into_iter()
            .flat_map(|(report, occurrences)| {
                occurrences.into_iter().filter_map(move |pointers| {
                    let loc = pointers[0].loc;
                    let path = loc.pathname();
                    (path == Path::new(EVENTS) || path == Path::new(TEMPLATE))
                        .then(|| (path, loc.line, report.clone()))
                })
            })
            .collect();
        if broken {
            assert_eq!(found.len(), 2, "{found:#?}");
            for (path, line, kind) in [(EVENTS, 3, "state"), (TEMPLATE, 2, "company")] {
                assert!(
                    found.iter().any(|(file, at, report)| {
                        *file == Path::new(path)
                            && *at == line
                            && report.key.to_string() == "scopes"
                            && report.msg.to_ascii_lowercase().contains(kind)
                    }),
                    "missing scope diagnostic at {path}:{line}: {found:#?}"
                );
            }
        } else {
            assert!(found.is_empty(), "{found:#?}");
        }
    }
}
