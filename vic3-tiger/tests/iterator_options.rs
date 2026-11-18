use std::fs;

use tiger_lib::{Everything, take_reports};

#[test]
fn vic3_iterator_tooltip_options() {
    let fixture = tempfile::tempdir().unwrap();
    let triggers = "common/scripted_triggers/iterator_options.txt";
    let effects = "common/scripted_effects/iterator_options.txt";
    for (path, script) in [
        (
            triggers,
            r"test_iterator_options = {
    any_country = { list_scopes = yes show_trigger_per_scope = yes always = yes }
    any_country = { list_scopes = no show_trigger_per_scope = no always = yes }
    any_country = { list_scopes = yes always = yes }
    any_country = { list_scopes = maybe show_trigger_per_scope = maybe always = yes }
    any_country = { show_trigger_per_scope = yes always = yes }
    any_country = { list_scopes = no show_trigger_per_scope = yes always = yes }
    list_scopes = yes
    show_trigger_per_scope = yes
}",
        ),
        (
            effects,
            r"test_iterator_options = {
    every_country = { list_scopes = yes show_trigger_per_scope = yes set_variable = example }
    random_country = { list_scopes = yes show_trigger_per_scope = yes set_variable = example }
    ordered_country = { list_scopes = yes show_trigger_per_scope = yes set_variable = example }
}",
        ),
    ] {
        let path = fixture.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("\u{feff}{script}")).unwrap();
    }

    let mut data = Everything::new(None, None, None, None, fixture.path(), Vec::new()).unwrap();
    data.load_all();
    data.validate_all();

    let reports = take_reports();
    let mut observed = Vec::new();
    for (report, locations) in &reports {
        for pointers in locations {
            let location = &pointers[0].loc;
            let path = location.pathname().to_str().unwrap().replace('\\', "/");
            if path == triggers || path == effects {
                let script = if path == triggers { triggers } else { effects };
                observed.push((script, location.line, report.msg.as_str()));
            }
        }
    }
    observed.sort_unstable();
    let dependency = "`show_trigger_per_scope` is only used when `list_scopes = yes` is also set";
    let list_placement = "unknown token `list_scopes`";
    let show_placement = "unknown token `show_trigger_per_scope`";
    assert_eq!(
        observed,
        [
            (effects, 2, list_placement),
            (effects, 2, show_placement),
            (effects, 3, list_placement),
            (effects, 3, show_placement),
            (effects, 4, list_placement),
            (effects, 4, show_placement),
            (triggers, 5, "expected yes or no"),
            (triggers, 5, "expected yes or no"),
            (triggers, 6, dependency),
            (triggers, 7, dependency),
            (triggers, 8, list_placement),
            (triggers, 9, show_placement),
        ]
    );
}
