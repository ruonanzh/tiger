use crate::datatype::{Arg, Args, Datatype, Vic3Datatype};
use crate::item::Item;

use Arg::*;
use Datatype::*;
use Vic3Datatype::*;

// The include/ files are converted from the game's data_type_* output files.
pub const GLOBAL_PROMOTES_VIC3: &[(&str, Args, Datatype)] =
    include!("include/data_global_promotes.rs");
pub const GLOBAL_FUNCTIONS_VIC3: &[(&str, Args, Datatype)] =
    include!("include/data_global_functions.rs");
pub const PROMOTES_VIC3: &[(&str, Datatype, Args, Datatype)] = include!("include/data_promotes.rs");
pub const FUNCTIONS_VIC3: &[(&str, Datatype, Args, Datatype)] =
    include!("include/data_functions.rs");

pub const GLOBAL_FUNCTIONS_VIC3_1_15_2: &[(&str, Args, Datatype)] =
    &[("GetNumSelectedFleetsMissionSeaRegions", Args::Args(&[]), int32)];
pub const FUNCTIONS_VIC3_1_15_2: &[(&str, Datatype, Args, Datatype)] = &[
    ("GetMaxConscriptBattalions", Vic3(Country), Args::Args(&[]), int32),
    ("GetNumStationSelectionsCapped", Vic3(MilitaryFormation), Args::Args(&[]), int32),
    ("GetWorldMarketHubTradingStates", Vic3(State), Args::Args(&[]), Unknown),
    ("HasCheckedMilitaryPanelFleets", Vic3(SeaNodeMovementMarker), Args::Args(&[]), bool),
    ("HasExports", Vic3(MarketPanel), Args::Args(&[]), bool),
    ("HasImports", Vic3(MarketPanel), Args::Args(&[]), bool),
    ("HasSelectionEditNavalMissionInteraction", Vic3(MapInteractionManager), Args::Args(&[]), bool),
    ("IsChecked", Vic3(MilitaryFormation), Args::Args(&[]), bool),
    ("IsNumStationSelectionsCapped", Vic3(MilitaryFormation), Args::Args(&[]), bool),
    ("IsSelectNavalMissionTypeForSeaNodeOpen", Vic3(RightClickMenuManager), Args::Args(&[]), bool),
    // The 1.15.2 export lists one argument without its type.
    ("OnSelectedFleetsRightClick", Vic3(Province), Args::Args(&[DType(Unknown)]), void),
    ("ToggleChecked", Vic3(MilitaryFormation), Args::Args(&[]), void),
    ("WouldCancelSelectedFleetMissions", Vic3(Province), Args::Args(&[]), bool),
    ("WouldRemoveSeaRegionFromSelectedFleetMissions", Vic3(Province), Args::Args(&[]), bool),
    ("WouldSelectNewMissionForSelectedFleets", Vic3(Province), Args::Args(&[]), bool),
];
