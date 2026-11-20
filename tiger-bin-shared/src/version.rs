//! Helper functions for handling the game's version compared to tiger's expected game version.

#[cfg(not(feature = "vic3"))]
use anyhow::Result;

#[cfg(feature = "vic3")]
pub fn show_vic3_version_banner(baseline: &str, installed: Option<&str>) {
    eprintln!(
        "Tiger Victoria 3 baseline: {baseline}; includes selected rule updates through 1.15.2."
    );
    if let Some(installed) = installed {
        eprintln!("Detected Victoria 3 version: {installed}.");
    } else {
        eprintln!("Could not detect the installed Victoria 3 version.");
    }
}

/// Compare the game version tiger was built for to the detected game version, and warn
/// on stderr if tiger is too old or the game is too old.
#[cfg(not(feature = "vic3"))]
pub fn warn_versions(
    tiger_game: &str,
    tiger_game_version: &str,
    launcher_game_version: &str,
) -> Result<()> {
    let components: Vec<_> = tiger_game_version.split(' ').collect();
    let tiger_version_components: Vec<_> = components[0].split('.').collect();
    let tiger_major: i64 = tiger_version_components.first().unwrap_or(&"0").parse()?;
    let tiger_minor: i64 = tiger_version_components.get(1).unwrap_or(&"0").parse()?;
    let tiger_patch: i64 = tiger_version_components.get(2).unwrap_or(&"0").parse()?;
    let game_version_components: Vec<_> = launcher_game_version.split('.').collect();
    let game_major: i64 = game_version_components.first().unwrap_or(&"0").parse()?;
    let game_minor: i64 = game_version_components.get(1).unwrap_or(&"0").parse()?;
    let game_patch: i64 = game_version_components.get(2).unwrap_or(&"0").parse()?;

    if (tiger_major, tiger_minor) < (game_major, game_minor) {
        eprintln!();
        eprintln!("PLEASE UPDATE!");
        eprintln!();
        eprintln!("Tiger was made for {tiger_game} version {tiger_game_version},");
        eprintln!("but the newer version {launcher_game_version} was detected in the game files.");
        eprintln!("This may lead to erroneous reports from Tiger.");
        eprintln!("Please check if there is a newer version of Tiger that supports this version.");
    } else if (tiger_major, tiger_minor) > (game_major, game_minor) {
        eprintln!();
        eprintln!("OLD GAME DETECTED");
        eprintln!();
        eprintln!("Tiger was made for {tiger_game} version {tiger_game_version},");
        eprintln!("but the older version {launcher_game_version} was detected in the game files.");
        eprintln!("This may lead to erroneous reports from Tiger.");
        eprintln!(
            "Please consider updating your game, or perhaps downgrading Tiger to a more compatible version."
        );
    } else if tiger_patch < game_patch {
        eprintln!("Tiger was made for {tiger_game} version {tiger_game_version},");
        eprintln!(
            "but the slightly newer version {launcher_game_version} was detected in the game files."
        );
        eprintln!("This is probably okay, but there may be some erroneous reports.");
    } else if tiger_patch > game_patch {
        eprintln!("Tiger was made for {tiger_game} version {tiger_game_version},");
        eprintln!(
            "but the slightly older version {launcher_game_version} was detected in the game files."
        );
        eprintln!("This is probably okay, but there may be some erroneous reports.");
    } else {
        eprintln!("Tiger was made for {tiger_game} version {tiger_game_version},");
        eprintln!("which matches the detected game version {launcher_game_version}.");
    }
    Ok(())
}
