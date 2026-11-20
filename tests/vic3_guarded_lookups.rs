#![cfg(feature = "vic3")]

use std::fs;

use tiger_lib::{Confidence, Everything, Game, Severity, take_reports};

const TRIGGERS: &str = "common/scripted_triggers/optional_lookup.txt";
const BAD_COMPARATOR: &str = "common/scripted_triggers/bad_comparator.txt";
const EFFECTS: &str = "common/scripted_effects/optional_lookup.txt";
const ACTIONS: &str = "common/diplomatic_actions/legacy_transfer.txt";

#[test]
fn vic3_guarded_lookups_and_legacy_transfers_are_advisory_at_their_own_locations() {
    Game::set(Game::Vic3).unwrap();
    let root = std::env::temp_dir().join(format!("vic3-tiger-guarded-{}", std::process::id()));
    for (file, text) in [
        (
            TRIGGERS,
            r"guard_check = {
    c:Z9Y ?= this
    c:Z9Y = this
    cd:Z9Y ?= { always = yes }
    s:STATE_GUARD_MISSING ?= { always = yes }
    s:STATE_GUARD_MISSING = { always = yes }
    has_state_in_state_region = STATE_GUARD_MISSING
    exists = c:Z9Y
    scope:guard_optional ?= { c:Z9Y = this }
    cx:Z9Y ?= this
    c: ?= this
    cu:guard_culture_missing ?= { always = yes }
}",
        ),
        (BAD_COMPARATOR, "guard_bad = { c:Z9Y ?== this }"),
        (
            EFFECTS,
            r"guard_effect = {
    c:Z9Y ?= { }
    c:Z9Y = { }
    s:STATE_GUARD_MISSING ?= { }
    s:STATE_GUARD_MISSING = { }
    region_state:Z9Y ?= { }
}",
        ),
        (
            ACTIONS,
            r"verify_legacy = {
    pact = {
        income_transfer_to_pops = {
            allow_discriminated = yes
            allow_discriminated = maybe
            allow_discriminated = { }
            allow_non_fully_accepted = typo
            lower_strata_pops = missing_guard_value
            typo = yes
        }
    }
    allow_discriminated = yes
}",
        ),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("\u{feff}{text}")).unwrap();
    }

    let mut data = Everything::new(None, None, None, None, &root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    let mut reports = Vec::new();
    for (report, occurrences) in take_reports() {
        for pointers in occurrences {
            let loc = pointers[0].loc;
            let path = loc.pathname().to_string_lossy().replace('\\', "/");
            reports.push((path, loc.line, report.clone()));
        }
    }
    let matching = |path, line, key, severity| {
        reports.iter().filter(move |(p, l, report)| {
            p == path && *l == line && report.key.to_string() == key && report.severity == severity
        })
    };

    for (path, line) in [(TRIGGERS, 2), (TRIGGERS, 4), (TRIGGERS, 5), (EFFECTS, 2), (EFFECTS, 4)] {
        let tips: Vec<_> = matching(path, line, "missing-item", Severity::Tips).collect();
        assert_eq!(tips.len(), 1, "{path}:{line} {reports:?}");
        assert_eq!(tips[0].2.confidence, Confidence::Weak);
        assert!(tips[0].2.info.as_deref().unwrap().contains("`?=`"));
        assert_eq!(matching(path, line, "missing-item", Severity::Error).count(), 0);
    }
    for (path, line) in [
        (TRIGGERS, 3),
        (TRIGGERS, 6),
        (TRIGGERS, 7),
        (TRIGGERS, 8),
        (TRIGGERS, 9),
        (TRIGGERS, 11),
        (TRIGGERS, 12),
        (EFFECTS, 3),
        (EFFECTS, 5),
        (EFFECTS, 6),
    ] {
        assert!(
            matching(path, line, "missing-item", Severity::Error)
                .any(|(_, _, report)| report.confidence == Confidence::Strong),
            "{path}:{line} {reports:?}"
        );
        assert_eq!(matching(path, line, "missing-item", Severity::Tips).count(), 0);
    }
    assert_eq!(matching(TRIGGERS, 10, "validation", Severity::Error).count(), 1);
    assert_eq!(matching(BAD_COMPARATOR, 1, "parse-error", Severity::Error).count(), 1);
    assert_eq!(matching(BAD_COMPARATOR, 1, "missing-item", Severity::Tips).count(), 0);

    for line in [4, 5, 6] {
        assert!(
            matching(ACTIONS, line, "validation", Severity::Tips).any(|(_, _, report)| {
                report.confidence == Confidence::Weak
                    && report.msg == "`allow_discriminated` compatibility needs verification"
            }),
            "{ACTIONS}:{line} {reports:?}"
        );
    }
    assert_eq!(matching(ACTIONS, 4, "validation", Severity::Error).count(), 0);
    for line in [5, 7] {
        assert!(
            matching(ACTIONS, line, "validation", Severity::Error)
                .any(|(_, _, report)| report.msg == "expected yes or no")
        );
    }
    assert_eq!(matching(ACTIONS, 6, "structure", Severity::Error).count(), 1);
    for line in [8, 9, 12] {
        assert_eq!(matching(ACTIONS, line, "unknown-field", Severity::Error).count(), 1);
    }
    assert_eq!(matching(ACTIONS, 12, "validation", Severity::Tips).count(), 0);
    assert!(
        !reports
            .iter()
            .any(|(p, _, report)| p == ACTIONS && report.key.to_string() == "duplicate-field")
    );
    fs::remove_dir_all(root).unwrap();
}
