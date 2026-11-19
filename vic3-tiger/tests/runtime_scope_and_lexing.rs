use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, Severity, take_reports};

const LOCALIZATION: &str = "localization/english/closure_l_english.yml";
const EVENTS: &str = "events/closure.txt";
const AI: &str = "common/ai_strategies/closure.txt";
const JOURNAL: &str = "common/journal_entries/closure.txt";
const WAR_GOAL: &str = "common/war_goal_types/closure.txt";
const INTERACTION: &str = "common/character_interactions/closure.txt";
const PRINCIPLE: &str = "common/power_bloc_principles/closure.txt";

#[test]
fn verifies_runtime_contracts_with_legal_bad_and_restored_source() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let config = root.join("fixture.conf");
    fs::write(&config, "scope_override = { var:closure_configured = flag }").unwrap();

    for broken in [false, true, false] {
        let localization = r#"l_english:
 quoted_word: "l'armée"
 quoted_substitution: "[ConcatIfNeitherEmpty('$quoted_word|q$', 'suffix')]"
 citation_and_ellipsis: "Printed source.[14] […] [GetPlayer.GetName]"
 numeric_prose: "Price is 12$. Reference [GetPlayer.GetName]."
 call_spacing: "[GetPlayer . GetName]"
 open_reference: "$quoted_word$ and @warning! [GetPlayer.GetName]"
 icon_recovery: "@warning! and [GetPlayer.GetName]"
 icon_at_end: "Warning @warning!"
 quoted_display_text: "[Concept('concept_state', 'State display')]"
 standalone_dollar: "Use $ as the currency symbol."
 natural_dollar_prose: "The $Legislative Program$ event."
"#;
        let events = r#"namespace = closure
closure.1 = {
    type = country_event
    hidden = yes
    immediate = {
        set_variable = { name = closure_configured value = flag:probe }
        set_variable = { name = closure_runtime value = flag:probe }
        set_variable = { name = closure_runtime value = year }
        if = { limit = { var:closure_runtime == 1 } }
        trigger_event = closure.2
    }
}
closure.2 = {
    type = country_event
    hidden = yes
    trigger = { is_subject = no }
}
"#;
        let ai = r#"ai_strategy_default = {
    wanted_naval_fortification_levels = {
        value = 0
        if = { limit = { state_population > 50000 } add = 1 }
    }
}
"#;
        let journal = r#"je_closure = {
    context = country
    immediate = {
        show_as_tooltip = { trigger_event = closure.1 }
        trigger_event = closure.1
    }
}
"#;
        let journal = if broken {
            journal.replace("context = country", "context = none")
        } else {
            journal.to_owned()
        };
        let war_goal = r#"war_goal_closure = {
    kind = force_nationalization
    possible = {
        has_strategic_region_interest_tier = { strategic_region = scope:target_region value >= 1 }
    }
}
"#;
        let interaction = r#"interaction_closure = {
    ai_will_do = { is_heir_of_own_country = no }
    should_ai_evaluate = { is_subject = no }
}
"#;
        let principle = r#"principle_closure = {
    ai_weight = {
        value = 1
        if = { limit = { current_cohesion_number < 50 } multiply = 0 }
    }
}
"#;
        let war_goal = if broken {
            war_goal.replace("scope:target_region", "scope:target_state")
        } else {
            war_goal.to_owned()
        };
        let interaction = if broken {
            interaction.replace("is_heir_of_own_country = no", "state_population > 1000")
        } else {
            interaction.to_owned()
        };
        let principle = if broken {
            principle.replace("current_cohesion_number < 50", "state_population > 1000")
        } else {
            principle.to_owned()
        };
        let (localization, events, ai) = if broken {
            (
                localization
                    .replace("|q$", "$")
                    .replace("[GetPlayer . GetName]", "[GetPlayer.Get Name]")
                    .replace("Printed source.[14]", "Printed source.[GetPlayer.GetNmae]")
                    .replace("Price is 12$.", "Missing macro $invalid_key.")
                    .replace(
                        "$quoted_word$ and @warning! [GetPlayer.GetName]",
                        "$quoted_word and @¡warning! [GetPlayer.GetName]",
                    )
                    .replace(
                        "@warning! and [GetPlayer.GetName]",
                        "@warning ! and [GetPlayer.GetNmae]",
                    )
                    .replace("Warning @warning!", "Warning @warning")
                    .replace("'State display'", "'$State display$'"),
                events
                    .replace(
                        "closure_configured value = flag:probe",
                        "closure_configured value = 1",
                    )
                    .replace(
                        "closure_runtime value = year",
                        "closure_runtime value = root.state_population",
                    )
                    .replace("trigger_event = closure.2", "ruler = { trigger_event = closure.2 }")
                    .replace(
                        "trigger = { is_subject = no }",
                        "trigger = { is_character_alive = no }",
                    ),
                ai.replace("state_population > 50000", "is_subject = no"),
            )
        } else {
            (localization.to_owned(), events.to_owned(), ai.to_owned())
        };

        for (relative, text) in [
            (LOCALIZATION, &*localization),
            (EVENTS, &*events),
            (AI, &*ai),
            (JOURNAL, &journal),
            (WAR_GOAL, &war_goal),
            (INTERACTION, &interaction),
            (PRINCIPLE, &principle),
        ] {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, format!("\u{feff}{text}")).unwrap();
        }

        let mut data = Everything::new(Some(&config), None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();
        let reports = take_reports();
        let at = |file: &str, line, message: &str, severity| {
            reports.iter().any(|(report, occurrences)| {
                report.msg.contains(message)
                    && report.severity == severity
                    && occurrences.iter().any(|pointers| {
                        let loc = pointers[0].loc;
                        loc.pathname() == Path::new(file) && loc.line == line
                    })
            })
        };
        let warning = Severity::Warning;
        let syntax = |line, message: &str| {
            reports.iter().any(|(report, occurrences)| {
                report.severity == Severity::Warning
                    && report.confidence == Confidence::Reasonable
                    && report.msg.contains(message)
                    && occurrences.iter().any(|pointers| {
                        let loc = pointers[0].loc;
                        loc.pathname() == Path::new(LOCALIZATION) && loc.line == line
                    })
            })
        };
        assert_eq!(syntax(7, "didn't recognize a key"), broken);
        assert_eq!(syntax(7, "expected icon name"), broken);
        assert_eq!(syntax(8, "expected !") || syntax(8, "expected `!`"), broken);
        assert_eq!(syntax(8, "unknown datafunction GetNmae"), broken);
        assert_eq!(syntax(9, "expected ! after icon name"), broken);
        assert_eq!(syntax(10, "didn't recognize a key"), broken);
        for line in [11, 12] {
            assert!(at(LOCALIZATION, line, "didn't recognize a key", Severity::Tips));
            assert!(!syntax(line, "didn't recognize a key"));
        }
        assert!(!at(LOCALIZATION, 4, "unknown datafunction 14", warning));
        assert!(!at(AI, 4, "state_population", warning));
        assert!(!at(INTERACTION, 2, "is_heir_of_own_country", warning));
        assert!(!at(PRINCIPLE, 4, "current_cohesion_number", warning));
        assert_eq!(
            at(LOCALIZATION, 3, "Substitution introduces an apostrophe", Severity::Tips),
            broken,
            "{reports:#?}",
        );
        assert!(!at(LOCALIZATION, 2, "expected `)`", warning));
        assert!(!at(LOCALIZATION, 2, "expected `]`", warning));
        assert_eq!(at(LOCALIZATION, 4, "unknown datafunction GetNmae", warning), broken);
        assert_eq!(at(LOCALIZATION, 5, "didn't recognize a key", warning), broken);
        assert_eq!(at(LOCALIZATION, 6, "expected `]`", warning), broken);
        assert_eq!(
            at(LOCALIZATION, 6, "whitespace between data function tokens", Severity::Tips),
            !broken
        );
        assert_eq!(at(AI, 4, "`is_subject` is for country", warning), broken);
        assert_eq!(at(WAR_GOAL, 4, "expected strategic region", warning), broken, "{reports:#?}");
        assert_eq!(at(INTERACTION, 2, "`state_population` is for state", Severity::Tips), broken);
        assert_eq!(at(PRINCIPLE, 4, "`state_population` is for state", Severity::Tips), broken);
        assert_eq!(at(EVENTS, 6, "expected flag", warning), broken);
        assert_eq!(at(EVENTS, 8, "state_population", warning), broken);
        assert_eq!(at(EVENTS, 10, "`closure.2` is for country", warning), broken);
        assert_eq!(
            at(
                EVENTS,
                16,
                "`is_character_alive` is for character but scope seems to be country",
                warning
            ),
            broken
        );
        assert_eq!(
            at(
                JOURNAL,
                4,
                "Event closure.1 declares country scope in tooltip preview",
                Severity::Tips
            ),
            broken
        );
        assert!(!at(JOURNAL, 4, "scope seems to be none", warning));
        assert_eq!(
            at(JOURNAL, 5, "`closure.1` is for country but scope seems to be none", warning),
            broken
        );
        assert!(!at(EVENTS, 16, "scope seems to be none", warning));
    }
}
