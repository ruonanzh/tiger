use std::fs;

use tiger_lib::{Everything, take_reports};

#[test]
fn vic3_container_contracts() {
    let fixture = tempfile::tempdir().unwrap();
    let effects = "common/scripted_effects/container_contracts.txt";
    let triggers = "common/scripted_triggers/container_contracts.txt";
    let guis = "common/scripted_guis/container_contracts.txt";
    for (path, script) in [
        (
            effects,
            r"container_contracts_effect = {
    create_container = {}
    create_container = {
        name = example
        tags = { alpha beta }
        parent = container:outer
        save_scope_as = created
        on_created = { add_tag = extra }
    }
    scope:created = {
        add_tag = extra
        remove_tag = beta
        set_variable = { name = example value = this.name }
        parent = { set_variable = parent_was_seen }
        create_container = { name = this.name }
    }
    create_container = { name = flag:example }
    set_variable = { name = dynamic_name value = flag:example }
    create_container = { name = var:dynamic_name }
    every_country = {
        every_container = {
            tag = alpha
            tags = { beta }
            parent = this.capital
            limit = { has_tag = extra }
            destroy_container = yes
        }
    }
    random_container = { tags = { beta } remove_tag = beta }
    ordered_container = { tag = alpha order_by = 1 destroy_container = yes }
    create_container = yes
    create_container = { name = container:example }
    create_container = { tags = alpha parent = flag:example }
    create_container = { on_created = { destroy_container = maybe } unknown_option = yes }
    create_container = { on_created = { remove_tag = invalid.tag } }
    every_container = { tag = { alpha } tags = beta parent = {} }
    every_container = { parent = 1 add_tag = { alpha } }
    every_country = { every_container = { destroy_container = yes } destroy_container = yes }
    container:example = { destroy_container = yes }
}",
        ),
        (
            triggers,
            r"container_contracts_trigger = {
    any_container = { tag = alpha tags = { beta } parent = container:outer count > 1 has_tag = alpha }
    any_country = { any_container = { parent = this.capital has_tag = beta } }
    any_container = yes
    any_container = { has_tag = { alpha } has_tag = invalid.tag }
    any_country = { has_tag = alpha }
    container:example = { has_tag = alpha }
}",
        ),
        (
            guis,
            r"container_contracts_gui = {
    scope = container
    effect = { destroy_container = yes }
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

    let mut observed = Vec::new();
    for (report, locations) in take_reports() {
        for pointers in locations {
            let location = &pointers[0].loc;
            let path = location.pathname().to_str().unwrap().replace('\\', "/");
            if path == effects || path == triggers || path == guis {
                let script = if path == effects {
                    effects
                } else if path == triggers {
                    triggers
                } else {
                    guis
                };
                observed.push((script, location.line, report.msg.clone()));
            }
        }
    }
    observed.sort_unstable();
    let observed: Vec<_> =
        observed.iter().map(|(path, line, msg)| (*path, *line, msg.as_str())).collect();
    assert_eq!(
        observed,
        [
            (effects, 31, "expected block, found value"),
            (effects, 32, "`container:example` produces container but expected flag"),
            (effects, 33, "`flag:example` produces flag but expected non-primitive scope"),
            (effects, 33, "expected block, found value"),
            (effects, 34, "expected just `destroy_container = yes`"),
            (effects, 34, "unknown field `unknown_option`"),
            (effects, 35, "expected a container tag here"),
            (effects, 36, "expected block, found value"),
            (effects, 36, "expected value, found block"),
            (effects, 36, "expected value, found block"),
            (effects, 37, "expected non-primitive scope"),
            (effects, 37, "expected value, found block"),
            (effects, 38, "`destroy_container` is for container but scope seems to be country"),
            (triggers, 4, "expected block, found value"),
            (triggers, 5, "expected a container tag here"),
            (triggers, 5, "expected value, found block"),
            (triggers, 6, "`has_tag` is for container but scope seems to be country"),
        ],
    );
}
