use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, Severity, take_reports};

const ALERTS: &str = "common/alert_types/scope_fixture.txt";
const SHIPS: &str = "common/ship_modifications/scope_fixture.txt";
const ARTICLES: &str = "common/treaty_articles/scope_fixture.txt";
const EVENTS: &str = "events/scope_fixture.txt";

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_ship_weights_treaty_article_triggers_and_typed_trigger_operands() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(root, "common/ideologies/scope_fixture.txt", "fixture_ideology = {}");
    write(root, "common/cultures/scope_fixture.txt", "fixture_culture = {}");
    write(
        root,
        "common/script_values/scope_fixture.txt",
        r#"fixture_ship_weight = { value = "ship_modification_market_demand_ratio(scope:ship_modification_type)" }
           fixture_bad_ship_weight = { value = "ship_modification_market_demand_ratio(scope:ship_modification_typo)" }"#,
    );

    write(
        root,
        "common/on_actions/scope_fixture.txt",
        "on_law_checkpoint_stall = { events = { scope_fixture.1 } }",
    );

    for broken in [false, true, false] {
        let alert = if broken { "is_subject = no" } else { "is_major_formation = yes" };
        write(
            root,
            ALERTS,
            &format!(
                "fixture_formation_alert = {{
                    script_context = player_country_formation
                    valid = {{ {alert} }}
                }}"
            ),
        );
        let ship = if broken { "fixture_bad_ship_weight" } else { "fixture_ship_weight" };
        write(root, SHIPS, &format!("fixture_ship = {{ ai_weight = {ship} }}"));
        let root_trigger = if broken { "is_subject = no" } else { "exists = root.input_goods" };
        write(
            root,
            ARTICLES,
            &format!(
                "fixture_article = {{
                    kind = directed
                    maintenance_paid_by = target_country
                    can_ratify = {{ {root_trigger} }}
                    can_propose = {{ {root_trigger} }}
                    ai = {{ evaluation_chance = fixture_ship_weight }}
                    requirement_to_maintain = {{
                        trigger = {{ {root_trigger} }}
                        show_about_to_break_warning = {{ {root_trigger} }}
                    }}
                }}"
            ),
        );
        let condition = if broken { "this" } else { "prev" };
        let culture_link = if broken { "owner" } else { "religion" };
        let ideology = if broken { "yes" } else { "ideology:fixture_ideology" };
        write(
            root,
            EVENTS,
            &format!(
                "namespace = scope_fixture
                scope_fixture.1 = {{
                    type = country_event
                    hidden = yes
                    trigger = {{
                        any_harvest_condition = {{
                            harvest_condition_intensity = {{ target = {condition} value > 5 }}
                        }}
                        any_political_movement = {{ has_core_ideology = {ideology} }}
                        exists = cu:fixture_culture.{culture_link}
                    }}
                    immediate = {{ add_treasury = fixture_ship_weight }}
                }}"
            ),
        );

        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();
        let mut scope_reports = Vec::new();
        for (report, occurrences) in take_reports() {
            if !matches!(
                report.key.to_string().as_str(),
                "scopes" | "strict-scopes" | "unknown-field"
            ) {
                continue;
            }
            for pointers in occurrences {
                let loc = &pointers[0].loc;
                for path in [ALERTS, SHIPS, ARTICLES, EVENTS] {
                    if loc.pathname() == Path::new(path) {
                        scope_reports.push((
                            path,
                            report.msg.clone(),
                            report.severity,
                            report.confidence,
                        ));
                    }
                }
            }
        }
        let reports_for = |path| scope_reports.iter().filter(|(p, ..)| *p == path).count();
        assert_eq!(reports_for(ALERTS), usize::from(broken), "{scope_reports:?}");
        assert_eq!(reports_for(SHIPS), usize::from(broken), "{scope_reports:?}");
        assert_eq!(reports_for(ARTICLES), if broken { 5 } else { 1 }, "{scope_reports:?}");
        // The same ship weight used outside a ship modification must still require its input.
        for consumer in [ARTICLES, EVENTS] {
            assert!(
                scope_reports.iter().any(|(path, msg, severity, confidence)| *path == consumer
                    && *severity == Severity::Tips
                    && *confidence == Confidence::Weak
                    && msg
                        == "`fixture_ship_weight` expects scope:ship_modification_type to be set"),
                "missing scope diagnostic in {consumer}: {scope_reports:?}"
            );
        }
        assert_eq!(reports_for(EVENTS), if broken { 4 } else { 1 }, "{scope_reports:?}");
    }
}
