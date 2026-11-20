#![cfg(any(feature = "ck3", feature = "vic3"))]

use std::path::Path;

use tiger_lib::{Everything, Game, Severity, take_reports};

fn check_iterator_comparators(tolerated: Severity) {
    let mod_root = Path::new("tests/files/iterator-comparator");
    let mut data = Everything::new(None, None, None, None, mod_root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let reports_at = |line| {
        reports.iter().filter_map(move |(report, pointers)| {
            pointers
                .iter()
                .any(|p| {
                    p[0].loc.pathname() == "events/iterator_comparator.txt" && p[0].loc.line == line
                })
                .then_some(report)
        })
    };

    let comparator_at = |line| {
        reports_at(line)
            .find(|report| report.msg.starts_with("expected `") && report.msg.contains("found `"))
            .map(|report| report.severity)
    };
    assert_eq!(comparator_at(6), None);
    assert_eq!(comparator_at(7), None);
    assert_eq!(comparator_at(8), Some(tolerated));
    for line in 12..=19 {
        assert_eq!(comparator_at(line), Some(Severity::Error), "line {line}");
    }
    assert_eq!(comparator_at(25), Some(Severity::Error));
    if Game::is_vic3() {
        assert_eq!(comparator_at(31), Some(Severity::Error));
        let scripted_comparators: Vec<_> = reports
            .iter()
            .filter(|(report, pointers)| {
                report.msg == "expected `random_interest_group =` or `?=`, found `<=`"
                    && pointers.iter().any(|p| {
                        p[0].loc.pathname() == "common/scripted_effects/iterator_comparator.txt"
                    })
            })
            .map(|(report, _)| report.severity)
            .collect();
        assert_eq!(scripted_comparators, [Severity::Error]);
        assert!(reports_at(8).any(|report| {
            report.severity == Severity::Warning
                && report.msg == "expected `random_interest_group =` or `?=`, found `<=`"
        }));
        assert!(
            reports_at(10).any(|report| {
                report.severity == Severity::Error && report.msg == "required field `name` missing"
            }),
            "the tolerated comparator must still validate the iterator body"
        );
        assert!(reports_at(17).any(|report| {
            report.severity == Severity::Error && report.msg == "expected block, found value"
        }));
    }
}

#[cfg(feature = "vic3")]
#[test]
fn vic3_random_interest_group_comparator() {
    Game::set(Game::Vic3).unwrap();
    check_iterator_comparators(Severity::Warning);
}

#[cfg(all(feature = "ck3", not(feature = "vic3")))]
#[test]
fn ck3_random_interest_group_comparator() {
    Game::set(Game::Ck3).unwrap();
    check_iterator_comparators(Severity::Error);
}
