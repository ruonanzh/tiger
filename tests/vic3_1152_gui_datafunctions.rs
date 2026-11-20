#![cfg(feature = "vic3")]

use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, LogReportMetadata, Severity, take_reports};

#[test]
fn vic3_1152_gui_datafunctions_validate_receivers_arguments_and_return_types() {
    Game::set(Game::Vic3).unwrap();
    let root =
        std::env::temp_dir().join(format!("vic3-tiger-gui-signatures-{}", std::process::id()));
    let gui = root.join("gui");
    fs::create_dir_all(&gui).unwrap();
    fs::write(
        gui.join("signatures.gui"),
        r#"template valid_signatures {
    visible = "[GreaterThan_int32(Country.GetMaxConscriptBattalions, GetNumSelectedFleetsMissionSeaRegions)]"
    visible = "[GreaterThan_int32(MilitaryFormation.GetNumStationSelectionsCapped, '(int32)0')]"
    visible = "[IsDataModelEmpty(State.GetWorldMarketHubTradingStates)]"
    visible = "[SeaNodeMovementMarker.HasCheckedMilitaryPanelFleets]"
    visible = "[Or(MarketPanel.HasExports, MarketPanel.HasImports)]"
    visible = "[MapInteractions.HasSelectionEditNavalMissionInteraction]"
    visible = "[Or(MilitaryFormation.IsChecked, MilitaryFormation.IsNumStationSelectionsCapped)]"
    visible = "[RightClickMenuManager.IsSelectNavalMissionTypeForSeaNodeOpen]"
    onrightclick = "[SeaNodeMovementMarker.GetSeaNode.OnSelectedFleetsRightClick(PdxGuiWidget.AccessSelf)]"
    onclick = "[MilitaryFormation.ToggleChecked]"
    visible = "[SeaNodeMovementMarker.GetSeaNode.WouldCancelSelectedFleetMissions]"
    visible = "[SeaNodeMovementMarker.GetSeaNode.WouldRemoveSeaRegionFromSelectedFleetMissions]"
    visible = "[SeaNodeMovementMarker.GetSeaNode.WouldSelectNewMissionForSelectedFleets]"
    visible = "[PlayerJoinRequest.IsChecked]"
}
template invalid_signatures {
    visible = "[Country.IsChecked]"
    onrightclick = "[Country.OnSelectedFleetsRightClick(PdxGuiWidget.AccessSelf)]"
    onrightclick = "[SeaNodeMovementMarker.GetSeaNode.OnSelectedFleetsRightClick]"
    onrightclick = "[SeaNodeMovementMarker.GetSeaNode.OnSelectedFleetsRightClick(PdxGuiWidget.AccessSelf, PdxGuiWidget.AccessSelf)]"
    visible = "[MilitaryFormation.IsChecked('(int32)0')]"
    visible = "[Not(Country.GetMaxConscriptBattalions)]"
    visible = "[MarketPanel.hasExports]"
    visible = "[GreaterThan_int32(getNumSelectedFleetsMissionSeaRegions, '(int32)0')]"
}
"#,
    )
    .unwrap();

    let current = validate(&root, Some("1.15.2"));
    let previous = validate(&root, Some("1.15.1"));
    let unknown = validate(&root, None);
    fs::remove_dir_all(root).unwrap();

    for diagnostics in [&previous, &unknown] {
        let unapplied: Vec<_> =
            diagnostics.iter().filter(|(line, _)| (2..=14).contains(line)).collect();
        assert_eq!(unapplied.len(), 16, "{diagnostics:?}");
        assert!(diagnostics.iter().all(|(line, _)| *line != 15), "{diagnostics:?}");
        for line in [24, 25] {
            let (_, report) = diagnostics.iter().find(|(n, _)| *n == line).unwrap();
            assert!(report.info.is_none(), "{report:?}");
        }
    }
    for (line, expected) in [
        (24, "did you mean HasExports?"),
        (25, "did you mean GetNumSelectedFleetsMissionSeaRegions?"),
    ] {
        let (_, report) = current.iter().find(|(n, _)| *n == line).unwrap();
        assert_eq!(report.info.as_deref(), Some(expected));
    }
    let actual: Vec<_> = current
        .iter()
        .filter(|(line, _)| *line < 24)
        .map(|(line, report)| (*line, report.severity, report.msg.as_str()))
        .collect();
    assert_eq!(
        actual,
        [
            (18, Severity::Warning, "IsChecked cannot follow a Country promote"),
            (19, Severity::Warning, "OnSelectedFleetsRightClick cannot follow a Country promote"),
            (
                20,
                Severity::Warning,
                "OnSelectedFleetsRightClick takes 1 arguments but was given 0 here"
            ),
            (
                21,
                Severity::Warning,
                "OnSelectedFleetsRightClick takes 1 arguments but was given 2 here"
            ),
            (22, Severity::Warning, "IsChecked takes 0 arguments but was given 1 here"),
            (
                23,
                Severity::Warning,
                "GetMaxConscriptBattalions returns int32 but a bool is needed here"
            ),
        ]
    );
}

fn validate(root: &Path, version: Option<&str>) -> Vec<(u32, LogReportMetadata)> {
    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.set_vic3_game_version(version);
    data.load_all();
    data.validate_all();

    let mut diagnostics = Vec::new();
    for (report, occurrences) in take_reports() {
        if report.key.to_string() == "datafunctions" {
            for pointers in occurrences {
                let loc = pointers[0].loc;
                if loc.pathname().ends_with("signatures.gui") {
                    diagnostics.push((loc.line, report.clone()));
                }
            }
        }
    }
    diagnostics.sort_by_key(|(line, _)| *line);
    diagnostics
}
