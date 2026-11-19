use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, Severity, take_reports};

fn write(root: &Path, relative: &str, script: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("﻿{script}")).unwrap();
}

#[test]
fn validates_fixed_contracts_and_keeps_coverage_advisory() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let workshop = root.join("workshop");
    write(
        root,
        "common/modifier_type_definitions/unit_probe.txt",
        "unit_offense_add = {} battle_casualties_mult = {}",
    );
    write(
        &workshop,
        "4100000000/common/scripted_triggers/addon.txt",
        "third_party_rule = { nested_candidate = {} invalid_external_body = yes }
# commented_candidate = {}",
    );
    write(
        root,
        "common/scripted_triggers/probe.txt",
        "provider_probe = {
    third_party_rule = { ANY = 5 }
    third_party_rule = { lowercase = 5 }
    nested_candidate = yes
    commented_candidate = yes
    missing_required_rule = yes
}",
    );

    write(
        root,
        "common/production_methods/probe.txt",
        "pm_present = { is_default = yes unlocking_technologies = { unknown_technology } }",
    );
    write(
        root,
        "common/production_method_groups/probe.txt",
        "pmg_probe = { texture = gfx/optional.dds production_methods = { pm_present pm_absent } }",
    );
    write(
        root,
        "common/modifier_type_definitions/probe.txt",
        "country_probe = { decimals = 2 decimals = no boolean = yes hidden = yess }",
    );
    write(root, "common/scripted_effects/probe.txt", "probe_effect = { make_independent = yes }");
    write(root, "common/script_values/probe.txt", "probe_value = { value = 1 probe_effect = yes }");
    write(
        root,
        "common/genes/probe.txt",
        "accessory_genes = { probe_gene = { probe_generated = {} } }",
    );
    write(
        root,
        "gfx/portraits/accessories/probe.txt",
        r#"probe_accessory = { entity = { required_tags = "probe_generated,probe_missing" } }"#,
    );
    write(root, "common/war_goal_types/probe.txt", "probe_wargoal = { kind = make_subject }");
    write(
        root,
        "common/ai_strategies/probe.txt",
        "probe_strategy = { wargoal_weights = { probe_wargoal = 1 make_subject = 1 probe_typo = 1 } }",
    );
    write(
        root,
        "events/probe.txt",
        r"namespace = probe
probe.1 = {
    type = country_event
    trigger = {
        probe_effect = yes
        is_interest_group_type = scope:ig
        is_diplomatic_play_participant = yes
        is_diplomatic_play_participant = scope:other
        is_diplomatic_play_committed_participant = yes
        unresolved_external_macro = { ARG = token }
        unresolved_compatibility_key = { field = token }
    }
    immediate = {
        make_independent = yes
        make_indepedent = yes
        has_variable = probe
        trigger_event = probe.1
        trigger_event = { id = probe.2 }
    }
}",
    );

    for (phase, invalid) in [("legal", false), ("bad", true), ("restored", false)] {
        let icon_frame = if invalid { "invalid_frame" } else { "2" };
        let duration = if invalid { "year = 2" } else { "years = 2" };
        let comparison = if invalid {
            "num_mutual_trade_route_levels_with_country(root)"
        } else {
            "relations:root"
        };
        let size = if invalid { "bad_pop_size_operation = 10" } else { "value = 10" };
        write(
            root,
            "common/scripted_effects/closure.txt",
            &format!(
                "probe_duration = {{ enable_temporary_hostilities = {{ country = ROOT type = naval duration = {{ {duration} }} }} }}\nprobe_create = {{ create_pop = {{ size = {{ {size} }} }} }}"
            ),
        );
        let modifier = if invalid { "battle_casualties_mult" } else { "unit_offense_add" };
        write(
            root,
            "common/mobilization_options/closure.txt",
            &format!("probe_option = {{ unit_modifier = {{ {modifier} = 0.1 }} }}"),
        );
        let video = if invalid { "gfx/probe_missing.bk2" } else { "probe_alias" };
        write(root, "gfx/probe_exists.bk2", "");
        write(
            root,
            "gfx/media_aliases/probe.txt",
            "probe_alias = { video = gfx/probe_missing.bk2 fallback = probe_terminal }
probe_terminal = { video = gfx/probe_exists.bk2 }",
        );
        write(
            root,
            "events/closure_movie.txt",
            &format!(
                "namespace = closure_movie\nclosure_movie.1 = {{
type = country_event
event_image = {{ video = {video} }}
option = {{ name = fixture_title }}
}}"
            ),
        );
        write(
            root,
            "gui/probe.gui",
            &format!(
                "REPLACE_OR_CREATE:texticon = {{ icon = probe_icon iconsize = {{ frame = {icon_frame} }} }}"
            ),
        );
        let technology =
            if invalid { "probe_removed_technology" } else { "probe_available_technology" };
        write(root, "common/technology/technologies/probe.txt", "probe_available_technology = {}");
        write(
            root,
            "common/scripted_triggers/closure.txt",
            &format!(
                "probe_typed = {{ has_technology_researched = {technology} year >= 1839 }}\nprobe_comparison = {{ prestige > \"{comparison}\" }}"
            ),
        );
        write(
            root,
            "common/script_values/00_override.txt",
            "REPLACE_OR_CREATE:probe_early = { value = 1 }",
        );
        write(
            root,
            "common/script_values/50_originals.txt",
            "probe_early = { original_early_was_validated = 1 }\nprobe_late = { original_late_was_validated = 1 }",
        );
        let selected_operation =
            if invalid { "selected_override_was_validated = 1" } else { "value = 2" };
        write(
            root,
            "common/script_values/99_override.txt",
            &format!("REPLACE_OR_CREATE:probe_late = {{ {selected_operation} }}"),
        );
        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();
        let phase_reports = take_reports();
        let has = |key: &str, term: &str, severity| {
            phase_reports.keys().any(|report| {
                report.key.to_string() == key
                    && report.msg.contains(term)
                    && report.severity == severity
            })
        };
        assert_eq!(
            has("missing-item", "technology probe_removed_technology", Severity::Error),
            invalid,
            "{phase}"
        );
        let malformed_frame = phase_reports.iter().any(|(report, occurrences)| {
            report.key.to_string() == "validation"
                && report.msg.contains("expected integer")
                && occurrences.iter().any(|pointers| {
                    pointers.iter().any(|pointer| pointer.loc.pathname().ends_with("gui/probe.gui"))
                })
        });
        assert_eq!(malformed_frame, invalid, "{phase}: {phase_reports:?}");
        assert_eq!(
            has(
                "removed",
                "`num_mutual_trade_route_levels_with_country` was removed in 1.9",
                Severity::Error
            ),
            invalid,
            "{phase}: {phase_reports:?}",
        );
        assert_eq!(
            has("field-missing", "must have 1 of days, weeks, months, or years", Severity::Error),
            invalid,
            "{phase}: {phase_reports:?}",
        );
        assert_eq!(
            has("unknown-field", "bad_pop_size_operation", Severity::Error),
            invalid,
            "{phase}: {phase_reports:?}",
        );
        assert!(has("validation", "create_pop size uses an inline script value", Severity::Tips));
        assert_eq!(
            has("modifiers", "battle_casualties_mult", Severity::Warning),
            invalid,
            "{phase}: {phase_reports:?}",
        );
        assert_eq!(
            has("missing-file", "file gfx/probe_missing.bk2 does not exist", Severity::Warning),
            invalid,
            "{phase}: {phase_reports:?}",
        );
        assert!(has("missing-file", "fallback video gfx/probe_exists.bk2 exists", Severity::Tips));
        assert_eq!(
            has("unknown-field", "selected_override_was_validated", Severity::Error),
            invalid,
            "{phase}: {phase_reports:?}"
        );
        for key in
            ["iconsize", "1839", "original_early_was_validated", "original_late_was_validated"]
        {
            assert!(
                !phase_reports
                    .keys()
                    .any(|report| report.key.to_string() == "unknown-field"
                        && report.msg.contains(key)),
                "{phase}: {key}"
            );
        }
        assert!(has(
            "prefixes",
            "plain script value loads after an explicit override",
            Severity::Tips
        ));
    }

    let mut data = Everything::new(None, None, Some(&workshop), None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    let reports = take_reports();
    let findings: Vec<_> = reports
        .iter()
        .flat_map(|(report, occurrences)| {
            occurrences.iter().map(move |pointers| (report, pointers))
        })
        .collect();

    for (key, message, severity) in [
        ("missing-item", "production method pm_absent", Severity::Error),
        ("missing-item", "technology unknown_technology", Severity::Error),
        ("missing-item", "event probe.2", Severity::Error),
        ("missing-item", "interest group scope:ig", Severity::Error),
        ("missing-file", "gfx/optional.dds", Severity::Tips),
        ("missing-item", "accessory tag probe_generated", Severity::Tips),
        ("missing-item", "accessory tag probe_missing", Severity::Warning),
        ("missing-item", "war goal type make_subject", Severity::Tips),
        ("missing-item", "war goal type probe_typo", Severity::Error),
        ("validation", "expected integer", Severity::Error),
        ("validation", "expected yes or no", Severity::Error),
        ("duplicate-field", "decimals", Severity::Tips),
        ("wrong-use", "is an effect, and can't be used in a trigger", Severity::Error),
        ("wrong-use", "cannot be used as a script value operation", Severity::Error),
        ("unknown-field", "unresolved_external_macro", Severity::Error),
        ("unknown-field", "external scripted trigger `third_party_rule`", Severity::Tips),
        ("unknown-field", "commented_candidate", Severity::Error),
        ("unknown-field", "nested_candidate", Severity::Error),
        ("unknown-field", "missing_required_rule", Severity::Error),
        ("unknown-field", "unresolved_compatibility_key", Severity::Error),
        ("wrong-use", "has_variable", Severity::Error),
        ("unknown-field", "make_indepedent", Severity::Error),
        ("removed", "legacy trigger", Severity::Tips),
        ("validation", "legacy trigger", Severity::Error),
    ] {
        assert!(
            findings.iter().any(|(report, _)| {
                report.key.to_string() == key
                    && report.msg.contains(message)
                    && report.severity == severity
            }),
            "missing {severity:?} {key} {message}: {findings:#?}"
        );
    }
    for (key, message) in [
        ("missing-item", "production method pm_present"),
        ("missing-item", "event probe.1"),
        ("unknown-field", "make_independent"),
        ("unknown-field", "invalid_external_body"),
        ("unknown-field", "is_diplomatic_play_committed_participant"),
    ] {
        assert!(
            !findings.iter().any(|(report, _)| {
                report.key.to_string() == key && report.msg.contains(message)
            })
        );
    }
}
