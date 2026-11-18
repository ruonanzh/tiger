use std::fs;
use std::path::Path;

use tiger_lib::{Everything, take_reports};

const WAR_GOALS: &str = "common/war_goal_types/mirror.txt";
const TRACK: &str = "music/music.txt";
const THEME: &str = "music/main_themes/music.txt";
const GROUP: &str = "common/journal_entry_groups/images.txt";
const OBJECTIVE: &str = "common/objectives/images.txt";
const CONCEPT: &str = "common/game_concepts/visibility.txt";

const VALID: [(&str, &str); 6] = [
    (
        WAR_GOALS,
        r#"mirror_target = { kind = custom icon = "gfx/fixture.dds" }
mirror_test = {
    kind = custom
    icon = "gfx/fixture.dds"
    mirrored_wargoal = {
        method = swap
        type = mirror_target
    }
}
territorial_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = territorial } }
liberate_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = liberate_loser } }
transfer_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = reverse_transfer } }
subjugate_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = subjugate } }
restore_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = restore_subject } }
reparations_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = reparations } }
no_mirror_test = { kind = custom icon = "gfx/fixture.dds" mirrored_wargoal = { method = no_mirror } }
"#,
    ),
    (
        TRACK,
        "mood_test = {\n    mood = yes\n    weight = { if = { limit = { is_subject = yes } add = 3 } }\n}\n",
    ),
    (THEME, "theme_test = {\n    can_be_interrupted = yes\n}\n"),
    (GROUP, "journal_group_test = {\n    illustration = \"gfx/fixture.dds\"\n}\n"),
    (OBJECTIVE, "objective_test = {\n    illustration = \"gfx/fixture.dds\"\n}\n"),
    (CONCEPT, "concept_visibility_test = {\n    shown_in_encyclopedia = no\n}\n"),
];

fn write_fixture(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("\u{feff}{contents}")).unwrap();
}

#[test]
fn validates_mirrored_war_goals_track_weights_illustrations_and_concept_visibility() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_fixture(root, "gfx/fixture.dds", "");

    let invalid = [
        VALID[0]
            .1
            .replace("type = mirror_target\n", "# type omitted\n")
            .replace("method = territorial", "method = territoral")
            .replace("method = liberate_loser", "method = swap type = mirror_missing")
            .replace("method = reverse_transfer", "method = swap typ = mirror_target")
            .replace("method = subjugate", "methd = subjugate")
            .replace("method = restore_subject", "method = swap type = {}")
            .replace(
                "mirrored_wargoal = { method = reparations }",
                "mirrored_wargoa = { method = reparations }",
            ),
        VALID[1].1.replace("is_subject", "is_female"),
        VALID[2].1.replace("can_be_interrupted = yes", "weight = 2"),
        VALID[3].1.replace("gfx/fixture.dds", "gfx/missing_group.dds"),
        VALID[4].1.replace("\"gfx/fixture.dds\"", "{}"),
        VALID[5].1.replace("= no", "= 0"),
    ];
    let expected = [
        (WAR_GOALS, 5, "required field `type`"),
        (WAR_GOALS, 10, "expected one of"),
        (WAR_GOALS, 11, "mirror_missing"),
        (WAR_GOALS, 12, "unknown field `typ`"),
        (WAR_GOALS, 12, "required field `type`"),
        (WAR_GOALS, 13, "unknown field `methd`"),
        (WAR_GOALS, 13, "required field `method`"),
        (WAR_GOALS, 14, "expected value, found block"),
        (WAR_GOALS, 15, "unknown field `mirrored_wargoa`"),
        (TRACK, 3, "character"),
        (THEME, 2, "weight"),
        (GROUP, 2, "missing_group.dds"),
        (OBJECTIVE, 2, "expected value, found block"),
        (CONCEPT, 2, "expected yes or no"),
    ];

    for malformed in [false, true, false] {
        for ((path, valid), invalid) in VALID.iter().zip(&invalid) {
            write_fixture(root, path, if malformed { invalid } else { valid });
        }
        let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
        data.load_all();
        data.validate_all();

        let mut observed = Vec::new();
        for (report, locations) in take_reports() {
            for pointers in locations {
                let loc = &pointers[0].loc;
                let path = loc.pathname().to_string_lossy().replace('\\', "/");
                // These synthetic items do not supply localization.
                if VALID.iter().any(|(file, _)| *file == path)
                    && !report.msg.contains("localization")
                {
                    observed.push((path, loc.line, report.msg.clone()));
                }
            }
        }
        let expected = if malformed { expected.as_slice() } else { &[] };
        assert_eq!(observed.len(), expected.len(), "{observed:?}");
        for &(path, line, fragment) in expected {
            assert!(
                observed
                    .iter()
                    .any(|(p, n, msg)| p == path && *n == line && msg.contains(fragment)),
                "missing {path}:{line} {fragment} in {observed:?}"
            );
        }
    }
}
