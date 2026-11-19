use std::path::Path;

use tiger_lib::{Confidence, Everything, Severity, take_reports};

#[test]
fn validates_wargoal_infamy_calculations_in_both_parents() {
    let mod_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/files/vic3-infamy");
    let mut data = Everything::new(None, None, None, None, &mod_path, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let mut diagnostics = Vec::new();
    let mut shared_scope_reports = Vec::new();
    for (report, occurrences) in &reports {
        for pointers in occurrences {
            let loc = &pointers[0].loc;
            let path = loc.pathname();
            let parent = if path == "common/war_goal_types/infamy.txt" {
                ("war goal", 2)
            } else if path == "common/treaty_articles/infamy.txt" {
                ("treaty article", 3)
            } else {
                continue;
            };
            if report.msg.starts_with("scope:")
                && report.msg.ends_with("might not be available here")
            {
                assert_eq!(report.severity, Severity::Tips);
                assert_eq!(report.confidence, Confidence::Weak);
            }
            if parent.0 == "treaty article"
                && report.msg == "scope:infamy_dynamic might not be available here"
            {
                assert_eq!(pointers.len(), 2, "{pointers:#?}");
                let source = &pointers[1].loc;
                assert_eq!((source.pathname(), source.line, source.column), (path, 3, 5));
                shared_scope_reports.push(loc.line);
            }
            if loc.line > parent.1 {
                diagnostics.push((
                    parent.0,
                    loc.line - parent.1,
                    report.severity,
                    report.msg.as_str(),
                ));
            }
        }
    }

    for parent in ["war goal", "treaty article"] {
        assert!(
            diagnostics.contains(&(
                parent,
                5,
                Severity::Tips,
                "scope:infamy_dynamic might not be available here",
            )),
            "{diagnostics:#?}"
        );
        assert!(
            diagnostics.contains(&(
                parent,
                7,
                Severity::Tips,
                "scope:infamy_other might not be available here",
            )),
            "{diagnostics:#?}"
        );
        assert!(
            diagnostics.contains(&(
                parent,
                10,
                Severity::Error,
                "unknown field `infamy_severitty`"
            )),
            "{diagnostics:#?}"
        );
    }
    shared_scope_reports.sort_unstable();
    assert_eq!(shared_scope_reports, [4, 8]);
    assert!(
        diagnostics.contains(&(
            "treaty article",
            2,
            Severity::Tips,
            "scope:own_market_goods might not be available here",
        )),
        "{diagnostics:#?}"
    );
    assert_eq!(diagnostics.len(), 8, "{diagnostics:#?}");
}
