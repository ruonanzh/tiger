use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, Severity, take_reports};

const EVENTS: &str = "events/runtime_diagnostics.txt";
const LOCA: &str = "localization/english/runtime_diagnostics_l_english.yml";
const EDITOR: &str = "gui/gui_editor/editor_probe.gui";
const MOD_GUI: &str = "gui/gui_editor/mod_probe.gui";

fn write(root: &Path, file: &str, contents: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn distinguishes_runtime_inference_from_static_contradictions() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let vanilla = fixture.path().join("vanilla");
    for folder in ["game", "clausewitz", "jomini"] {
        fs::create_dir_all(vanilla.join(folder)).unwrap();
    }
    let root = &fixture.path().join("mod");
    write(
        &vanilla.join("clausewitz"),
        EDITOR,
        r#"widget = {
    text = "[GuiEditorCategory.AddProperty]"
    datacontext = "[GuiEditorProperty]"
}"#,
    );
    write(
        root,
        MOD_GUI,
        r#"widget = {
    text = "[GuiEditorProperty.NonexistentAuditMember]"
    visible = "[GuiEditor.NonexistentAuditMember]"
}"#,
    );
    write(
        root,
        "common/journal_entries/runtime_diagnostics.txt",
        "je_runtime_diagnostics = { context = country immediate = { trigger_event = runtime_diagnostics.1 } }",
    );
    write(
        root,
        EVENTS,
        r#"namespace = runtime_diagnostics
runtime_diagnostics.1 = {
    type = country_event
    hidden = yes
    immediate = {
        save_temporary_scope_as = saved_temporary
        set_variable = runtime_var trigger_event = runtime_diagnostics.2
    }
}
runtime_diagnostics.2 = {
    type = country_event
    hidden = yes
    trigger = {
        scope:saved_temporary ?= this
        scope:saved_temporary = this
        always = yes
        scope:country = { is_character_alive = yes }
        root = { is_character_alive = yes }
        scope:runtime_guess = { is_subject = no }
        scope:runtime_guess = { is_character_alive = yes }
        local_var:missing_local = { is_subject = no }
        is_subject_of_and_not_being_freed = var:runtime_var
        var:runtime_var = { is_subject = no }
        is_subject_of_and_not_being_freed = VAR:configured_var
        has_any_strait_control = no
        has_any_strait_province = yes
        any_scope_state = { has_any_strait_province = no }
        has_any_strait_province = c:BEL
        scope:formation = { is_on_front = yes }
        is_coastal = yes
        any_scope_state = { is_coastal = no }
        ruler = { is_coastal = yes }
    }
}"#,
    );
    write(
        root,
        LOCA,
        r#"l_english:
 runtime_scope: "[ROOT.GetName]"
 fixed_country: "[GetPlayer.GetCountry]"
 wrong_method: "[ROOT.GetNmae]"
 runtime_arg: "[ROOT.GetLeader('extra')]"
 working: "[ROOT.GetCountry.GetName]"
 known_engine_context: "[GuiEditorCategory.AddProperty]"
 misspelled_context: "[GuiEditorCatgry.AddProperty]"
 fixed_second_receiver: "[ROOT.GetRuler.GetString]"
 valid_second_receiver: "[ROOT.GetRuler.GetFullName]"
 ambiguous_return: "[ROOT.GetLeader.GetName]"
 bracket_key: "["
 malformed_bracket: "[ROOT.GetCountry"
 unknown_accessor: "[SCOPE.sInterestGroup('ig').GetName]"
 arg_width: "[IntToFixedPoint(GetPlayer.GetNumInterestGroupMembers)]"
 invalid_numeric_receiver: "[IntToFixedPoint(GetPlayer.GetName)]"
"#,
    );

    let lobbies = "common/political_lobbies/runtime_diagnostics.txt";
    write(
        root,
        lobbies,
        "lobby_runtime_diagnostics = {
    can_create = {
        is_subject = no
        scope:target_country = { is_character_alive = yes }
    }
    available_for_interest_group = { is_subject = no }
}",
    );

    write(root, "runtime-test.conf", "scope_override = { var:configured_var = flag }");
    let config_path = root.join("runtime-test.conf");
    let mut data =
        Everything::new(Some(&config_path), Some(&vanilla), None, None, root, Vec::new()).unwrap();
    data.load_all();
    tiger_lib::set_show_vanilla(true);
    data.validate_all();

    let reports = take_reports();
    let rows: Vec<_> = reports
        .iter()
        .flat_map(|(report, occurrences)| {
            occurrences.iter().map(move |pointers| (report, pointers[0].loc))
        })
        .collect();
    let expect = |file, line, key, text: &str, severity, confidence| {
        assert!(
            rows.iter().any(|(report, loc)| {
                loc.pathname() == Path::new(file)
                    && loc.line == line
                    && report.key.to_string() == key
                    && report.msg.contains(text)
                    && report.severity == severity
                    && report.confidence == confidence
            }),
            "missing {severity:?} {key} at {file}:{line} with {text:?}: {rows:#?}"
        );
    };
    for (line, key, text, severity, confidence) in [
        (14, "temporary-scope", "was temporary", Severity::Tips, Confidence::Weak),
        (15, "temporary-scope", "was temporary", Severity::Error, Confidence::Weak),
        (17, "strict-scopes", "scope:country might not", Severity::Tips, Confidence::Weak),
        (17, "scopes", "scope seems to be country", Severity::Tips, Confidence::Weak),
        (18, "scopes", "scope seems to be country", Severity::Warning, Confidence::Reasonable),
        (19, "strict-scopes", "scope:runtime_guess might not", Severity::Tips, Confidence::Weak),
        (20, "scopes", "scope seems to be country", Severity::Tips, Confidence::Weak),
        (21, "strict-scopes", "local_var:missing_local", Severity::Error, Confidence::Weak),
        (23, "scopes", "var:runtime_var was deduced", Severity::Tips, Confidence::Weak),
        (
            24,
            "scopes",
            "produces flag but expected country",
            Severity::Warning,
            Confidence::Reasonable,
        ),
    ] {
        expect(EVENTS, line, key, text, severity, confidence);
    }
    for (line, text, severity, confidence) in [
        (2, "receiver for GetName", Severity::Tips, Confidence::Weak),
        (3, "GetCountry cannot follow", Severity::Warning, Confidence::Reasonable),
        (4, "unknown datafunction GetNmae", Severity::Warning, Confidence::Reasonable),
        (5, "GetLeader takes 0 arguments", Severity::Warning, Confidence::Reasonable),
        (7, "unknown datafunction GuiEditorCategory", Severity::Warning, Confidence::Reasonable),
        (8, "unknown datafunction GuiEditorCatgry", Severity::Warning, Confidence::Reasonable),
        (9, "receiver for GetRuler", Severity::Tips, Confidence::Weak),
        (9, "GetString cannot follow a Character", Severity::Warning, Confidence::Reasonable),
        (10, "receiver for GetRuler", Severity::Tips, Confidence::Weak),
        (11, "receiver for GetLeader", Severity::Tips, Confidence::Weak),
        (14, "unknown datafunction sInterestGroup", Severity::Warning, Confidence::Reasonable),
        (15, "returns int64 but a int32", Severity::Tips, Confidence::Weak),
        (16, "returns CString but a int32", Severity::Warning, Confidence::Reasonable),
    ] {
        expect(LOCA, line, "datafunctions", text, severity, confidence);
    }
    for (path, line, receiver, severity, confidence) in [
        (EDITOR, 2, "GuiEditorCategory", Severity::Tips, Confidence::Weak),
        (EDITOR, 3, "GuiEditorProperty", Severity::Tips, Confidence::Weak),
        (MOD_GUI, 2, "GuiEditorProperty", Severity::Warning, Confidence::Reasonable),
        (MOD_GUI, 3, "GuiEditor", Severity::Warning, Confidence::Reasonable),
    ] {
        let message = format!("unknown datafunction {receiver}");
        expect(path, line, "datafunctions", message.as_str(), severity, confidence);
    }
    expect(lobbies, 3, "scopes", "is_subject", Severity::Tips, Confidence::Weak);
    expect(lobbies, 4, "scopes", "is_character_alive", Severity::Warning, Confidence::Reasonable);
    expect(lobbies, 6, "scopes", "is_subject", Severity::Warning, Confidence::Reasonable);
    expect(LOCA, 13, "datafunctions", "expected `]`", Severity::Warning, Confidence::Reasonable);
    expect(EVENTS, 28, "scopes", "but expected bool", Severity::Warning, Confidence::Reasonable);
    expect(EVENTS, 32, "scopes", "is_coastal", Severity::Warning, Confidence::Reasonable);
    assert!(!rows.iter().any(|(report, loc)| {
        loc.pathname() == Path::new(EVENTS)
            && report.key.to_string() == "scopes"
            && matches!(loc.line, 25..=27 | 30 | 31)
    }));
    assert!(!rows.iter().any(|(report, loc)| {
        loc.pathname() == Path::new(LOCA)
            && report.key.to_string() == "datafunctions"
            && (matches!(loc.line, 6 | 12)
                || (matches!(loc.line, 10 | 11) && report.severity != Severity::Tips))
    }));
}
