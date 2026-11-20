#![cfg(feature = "vic3")]

use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, LogReportMetadata, take_reports};

const GROUPS: &str = "common/law_groups/regime_change.txt";
const LAWS: &str = "common/laws/regime_change.txt";
const DEFINES: &str = "common/defines/regime_change.txt";
const EVENTS: &str = "events/regime_change.txt";

const GOOD_GROUPS: &str = r"
fixture_group = {
    regime_change_min_progressiveness_difference = 50
    change_allowed_trigger = {
        can_regime_change = c:FXT
        is_character_interaction_available = fixture_interaction
        any_military_formation = { has_naval_mission_without_area = no }
        any_scope_character = { is_character_interaction_available = fixture_interaction }
    }
}
";

const GOOD_DEFINES: &str = r"
NAI = { NUM_DAYS_TO_REMEMBER_FAILED_TRANSPORT_FLEET = 7 }
NGUI = { MAX_STATION_SELECTIONS_IN_FORMATION_LIST = 25 }
";

const GOOD_EVENTS: &str = r"
namespace = fixture
fixture.1 = {
    type = country_event
    hidden = yes
    immediate = {
        conclude_journal_entry = { type = fixture_journal outcome = runtime_effect_header }
        conclude_journal_entry = { type = fixture_journal outcome = runtime_effect_header hidden = yes }
    }
}
";

type Reports = Vec<(String, u32, LogReportMetadata)>;

fn write_fixture(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{text}")).unwrap();
}

fn validate(
    root: &Path,
    version: &str,
    groups: &str,
    laws: &str,
    defines: &str,
    events: &str,
) -> Reports {
    let mod_root = root.join("mod");
    for (path, text) in [
        (GROUPS, groups),
        (LAWS, laws),
        (DEFINES, defines),
        (EVENTS, events),
        ("common/country_definitions/fixture.txt", "FXT = {}"),
        ("common/character_interactions/fixture.txt", "fixture_interaction = {}"),
        (
            "common/journal_entries/fixture.txt",
            r"
fixture_journal = {
    outcome = {
        flags = { hidden timeout }
        effect_header = runtime_effect_header
        conclusion_desc = runtime_effect_header
    }
    outcome = {
        flags = { hidden invalid_outcome }
        effect_header = invalid_effect_header
        conclusion_desc = invalid_effect_header
        trigger = { always = no }
    }
    outcome = {
        flags = { hidden }
        effect_header = runtime_effect_header
        conclusion_desc = runtime_effect_header
        trigger = { always = yes }
    }
}
other_journal = {
    outcome = {
        flags = { hidden timeout }
        effect_header = timeout_effect_header
        conclusion_desc = timeout_effect_header
    }
    outcome = {
        flags = { hidden }
        effect_header = other_effect_header
        conclusion_desc = other_effect_header
        trigger = { always = yes }
    }
}",
        ),
        (
            "localization/english/fixture_l_english.yml",
            "l_english:\n fixture_group: \"Group\"\n fixture_group_desc: \"Group desc\"\n fixture_law: \"Law\"\n fixture_law_desc: \"Law desc\"\n runtime_effect_header: \"Outcome\"\n other_effect_header: \"Other outcome\"\n invalid_effect_header: \"Invalid outcome\"\n timeout_effect_header: \"Timeout outcome\"\n",
        ),
    ] {
        write_fixture(&mod_root, path, text);
    }
    let mut data = Everything::new(None, None, None, None, &mod_root, Vec::new()).unwrap();
    data.set_vic3_game_version(Some(version));
    data.load_all();
    data.validate_all();

    let mut reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            if [GROUPS, LAWS, DEFINES, EVENTS].contains(&path.as_str()) {
                reports.push((path, loc.line, report.clone()));
            }
        }
    }
    reports
}

#[test]
fn vic3_1_15_2_names_validate_arguments_and_recover_after_correction() {
    Game::set(Game::Vic3).unwrap();
    let root = std::env::temp_dir().join(format!("vic3-tiger-152-names-{}", std::process::id()));
    let good_laws = "fixture_law = { group = fixture_group }";
    let clean = validate(&root, "1.15.2", GOOD_GROUPS, good_laws, GOOD_DEFINES, GOOD_EVENTS);
    assert!(clean.is_empty(), "{clean:?}");

    let bad_groups = r"
fixture_group = {
    regime_change_min_progressiveness_difference = yes
    regime_change_min_progressiveness_diference = 25
    change_allowed_trigger = {
        can_regime_change = yes
        has_naval_mission_without_area = yes
        is_character_interaction_available = missing_interaction
        any_military_formation = { has_naval_mission_without_area = {} }
        any_scope_character = { can_regime_change = root }
        any_military_formation = { is_character_interaction_available = fixture_interaction }
        can_regime_chang = this
    }
}
";
    let bad_laws =
        "fixture_law = { group = fixture_group regime_change_min_progressiveness_difference = 50 }";
    let bad_defines = r"
NAI = { NUM_DAYS_TO_REMEMBER_FAILED_TRANSPORT_FLEET = yes }
NGUI = { MAX_STATION_SELECTIONS_IN_FORMATION_LIST = {} }
NGUI = { MAX_STATION_SELECTIONS_IN_FORMATION_LIS = 25 }
NGUI = { NUM_DAYS_TO_REMEMBER_FAILED_TRANSPORT_FLEET = 7 }
";
    let bad_events = r"
namespace = fixture
fixture.1 = {
    type = country_event
    hidden = yes
    immediate = {
        conclude_journal_entry = fixture_journal
        conclude_journal_entry = { type = missing_journal outcome = runtime_effect_header }
        conclude_journal_entry = { type = fixture_journal outcome = {} hidden = maybe }
        conclude_journal_entry = { type = fixture_journal }
        conclude_journal_entry = { outcome = runtime_effect_header }
        every_scope_character = {
            conclude_journal_entry = { type = fixture_journal outcome = runtime_effect_header }
        }
        conclude_journal_entry = { type = fixture_journal outcome = other_effect_header }
        conclude_journal_entry = { type = fixture_journal outcome = scope:dynamic_outcome }
        conclude_journal_entry = { type = scope:dynamic_journal outcome = runtime_effect_header }
        conclude_journal_entry = { type = fixture_journal outcome = invalid_effect_header }
        conclude_journal_entry = { type = other_journal outcome = timeout_effect_header }
    }
}
";
    let reports = validate(&root, "1.15.2", bad_groups, bad_laws, bad_defines, bad_events);
    for (file, line, fragment) in [
        (GROUPS, 3, "expected number"),
        (GROUPS, 4, "unknown field"),
        (GROUPS, 6, "country"),
        (GROUPS, 7, "military"),
        (GROUPS, 8, "missing_interaction"),
        (GROUPS, 9, "expected value"),
        (GROUPS, 10, "country"),
        (GROUPS, 11, "country"),
        (GROUPS, 12, "can_regime_chang"),
        (LAWS, 1, "unknown field"),
        (DEFINES, 2, "expected number"),
        (DEFINES, 3, "expected value"),
        (DEFINES, 4, "unknown define"),
        (DEFINES, 5, "unknown define"),
        (EVENTS, 7, "expected block"),
        (EVENTS, 8, "missing_journal"),
        (EVENTS, 9, "expected value"),
        (EVENTS, 9, "expected yes or no"),
        (EVENTS, 10, "outcome"),
        (EVENTS, 11, "type"),
        (EVENTS, 13, "country"),
        (EVENTS, 15, "has no outcome with effect_header other_effect_header"),
    ] {
        assert!(
            reports.iter().any(|(path, loc_line, report)| {
                path == file
                    && *loc_line == line
                    && report.msg.to_ascii_lowercase().contains(fragment)
            }),
            "missing {fragment:?} at {file}:{line}: {reports:?}"
        );
    }

    let effect_reports_at = |line| {
        reports.iter().filter(move |(path, loc_line, _)| path == EVENTS && *loc_line == line)
    };
    let missing = effect_reports_at(8).map(|(_, _, report)| report).collect::<Vec<_>>();
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].key.to_string(), "missing-item");
    assert_eq!(missing[0].severity, tiger_lib::Severity::Error);

    let foreign_outcome = effect_reports_at(15).map(|(_, _, report)| report).collect::<Vec<_>>();
    assert_eq!(foreign_outcome.len(), 1, "{foreign_outcome:?}");
    assert_eq!(foreign_outcome[0].severity, tiger_lib::Severity::Error);

    for line in [16, 17, 18, 19] {
        let review = effect_reports_at(line).map(|(_, _, report)| report).collect::<Vec<_>>();
        assert_eq!(review.len(), 1, "{reports:?}");
        assert_eq!(review[0].severity, tiger_lib::Severity::Tips);
        assert_eq!(review[0].confidence, tiger_lib::Confidence::Weak);
        assert!(review[0].msg.contains("cannot verify"), "{review:?}");
    }

    for (version, fragment) in [("1.15.1", "was introduced"), ("unknown", "version is unknown")] {
        let reports = validate(&root, version, GOOD_GROUPS, good_laws, GOOD_DEFINES, GOOD_EVENTS);
        assert_eq!(reports.len(), 2, "{reports:?}");
        assert!(
            reports.iter().all(|(file, _, report)| {
                file == EVENTS
                    && report.severity == tiger_lib::Severity::Tips
                    && report.msg.contains(fragment)
            }),
            "{reports:?}"
        );
    }

    let recovered = validate(&root, "1.15.2", GOOD_GROUPS, good_laws, GOOD_DEFINES, GOOD_EVENTS);
    assert!(recovered.is_empty(), "{recovered:?}");
    fs::remove_dir_all(root).unwrap();
}
