use std::fs;
use std::path::Path;

use tiger_lib::{Everything, Game, take_reports};

fn write_fixture(root: &Path, path: &str, contents: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    if file.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt")) {
        fs::write(file, format!("\u{feff}{contents}")).unwrap();
    } else {
        fs::write(file, contents).unwrap();
    }
}

#[test]
fn event_image_texture_resolves_aliases_and_files() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let vanilla = root.join("vanilla");
    let mod_root = root.join("mod");
    let aliases = "gfx/media_aliases/event_texture.txt";
    let events = "events/event_texture.txt";

    fs::create_dir_all(vanilla.join("clausewitz")).unwrap();
    fs::create_dir_all(vanilla.join("jomini")).unwrap();
    let vanilla_game = vanilla.join("game");
    write_fixture(&vanilla_game, aliases, "still = { texture = gfx/event_pictures/replaced.dds }");
    write_fixture(
        &mod_root,
        aliases,
        concat!(
            "still = { texture = gfx/event_pictures/still.dds video = gfx/event_pictures/clip.bk2 }\n",
            "missing = { texture = gfx/event_pictures/missing_alias.dds }\n",
            "video_only = { video = gfx/event_pictures/clip.bk2 }\n",
        ),
    );
    write_fixture(&mod_root, "gfx/event_pictures/still.dds", "");
    write_fixture(&mod_root, "gfx/event_pictures/clip.bk2", "");
    write_fixture(
        &mod_root,
        events,
        concat!(
            "namespace = image_test\n",
            "image_test.1 = { hidden = yes event_image = { texture = gfx/event_pictures/still.dds } }\n",
            "image_test.2 = { hidden = yes event_image = { texture = still } }\n",
            "image_test.3 = { hidden = yes event_image = { texture = typo } }\n",
            "image_test.4 = { hidden = yes event_image = { texture = gfx/event_pictures/missing_direct.dds } }\n",
            "image_test.5 = { hidden = yes event_image = { texture = missing } }\n",
            "image_test.6 = { hidden = yes event_image = { texture = video_only } }\n",
        ),
    );

    let mut data =
        Everything::new(None, Some(&vanilla), None, None, &mod_root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    let reports = take_reports();

    let mut event_reports = Vec::new();
    for (report, occurrences) in reports {
        for pointers in occurrences {
            if pointers[0].loc.pathname() == events {
                event_reports.push((
                    pointers[0].loc.line,
                    report.msg.clone(),
                    pointers.get(1).map(|p| (p.loc.pathname().to_path_buf(), p.loc.line)),
                ));
            }
        }
    }
    event_reports.sort_unstable();
    assert_eq!(
        event_reports,
        vec![
            (4, "media alias typo not defined in gfx/media_aliases/".into(), None),
            (5, "file gfx/event_pictures/missing_direct.dds does not exist".into(), None),
            (
                6,
                "texture file gfx/event_pictures/missing_alias.dds for media alias missing does not exist"
                    .into(),
                Some((aliases.into(), 2)),
            ),
            (7, "media alias video_only does not define a texture".into(), Some((aliases.into(), 3))),
        ],
    );
}
