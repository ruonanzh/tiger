use std::fs;
use std::path::Path;

use tiger_lib::{Confidence, Everything, Game, Severity, take_reports};

const ALIASES: &str = "gfx/media_aliases/fixture.txt";
const FONT: &str = "fonts/fixture.font";
const PM: &str = "common/production_methods/fixture.txt";
const VALUES: &str = "common/script_values/fixture.txt";
const MARKUP: &str = "localization/english/fixture_l_english.yml";
const MESH: &str = "gfx/models/fixture.asset";
const DDS_ORIGINAL: &str = "gfx/models/alpha/identical.dds";
const DDS_COPY: &str = "gfx/models/beta/identical.dds";
const DDS_DISTINCT_ORIGINAL: &str = "gfx/models/alpha/distinct.dds";
const DDS_DISTINCT_COPY: &str = "gfx/models/beta/distinct.dds";

fn write(root: &Path, file: &str, contents: impl AsRef<[u8]>) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn reports_diagnostic_evidence_without_changing_affected_rules() {
    Game::set(Game::Vic3).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(
        root,
        ALIASES,
        concat!(
            "\u{feff}main = { video = gfx/missing.bk2 fallback = intermediate }\n",
            "intermediate = { fallback = terminal }\n",
            "terminal = { video = gfx/Existing.BK2 }\n",
            "wrong_case = { video = gfx/existing.bk2 }\n",
            "truly_absent = { video = gfx/absent.bk2 fallback = missing_alias }\n",
            "cycle_a = { video = gfx/cycle.bk2 fallback = cycle_b }\n",
            "cycle_b = { fallback = cycle_a }\n",
            "main_texture = { video = gfx/missing_scene.bk2 fallback = texture_terminal }\n",
            "texture_terminal = { video = gfx/absent_terminal.bk2 texture = gfx/8x8.dds }\n",
        ),
    );
    write(root, "gfx/Existing.BK2", "");
    write(
        root,
        FONT,
        "fontfiles = { name = fixture group = { files = { fonts/NotoSerif/NotoSansKR-Medium.otf } } }",
    );
    write(root, "fonts/NotoSans/NotoSansKR-Medium.otf", "");
    write(root, "fonts/NotoSerif/NotoSerifKR-Medium.otf", "");
    write(
        root,
        VALUES,
        concat!(
            "\u{feff}\u{feff}numeric_round = { value = 2 round = 1 }\n",
            "\u{feff}numeric_floor = { value = 2 floor = 1.5 }\n",
            "bad_round = { value = 2 round = typo }\n",
            "unconditional = { value = 2 if = { add = 1 } }\n",
            "conditional = { value = 2 if = { limit = { always = yes } add = 1 } }\n",
        ),
    );
    write(
        root,
        PM,
        concat!(
            "\u{feff}fixture = { building_modifiers = { workforce_scaled = {\n",
            "    unit_offense_mult = 0.1\n",
            "    ship_accuracy_mult = 0.1\n",
            "    country_authority_add = 1\n",
            "    building_throughput_add = 0.1\n",
            "    ship_armor_mult = 0.1\n",
            "    unknown_modifier_shape = 0.1\n",
            "} } }\n",
        ),
    );
    write(
        root,
        "common/modifier_type_definitions/fixture.txt",
        "\u{feff}unit_offense_mult = {}\nship_accuracy_mult = {}\ncountry_authority_add = {}\nbuilding_throughput_add = {}",
    );
    write(
        root,
        MARKUP,
        concat!(
            "\u{feff}l_english:\n",
            " nested_tags: \"#r#b text#!#!\"\n",
            " attached_text: \"#Sreally#!\"\n",
            " dynamic_tooltip: \"#tooltippable;tooltip:[GetPlayer.GetTooltipTag],fixture_tooltip: text#!\"\n",
            " static_tooltip: \"#tooltippable;tooltip:fixture_tooltip: text#!\"\n",
            " fixture_tooltip: \"Example\"\n",
            " malformed_tag: \"#color:{1.0,0.0,0.0 unclosed\"\n",
        ),
    );
    write(
        root,
        MESH,
        concat!(
            "\u{feff}pdxmesh = { name = fixture_mesh file = \"fixture.mesh\"\n",
            "  meshsettings = { name = fixture_shape index = 0\n",
            "    texture_normal = \"identical.dds\"\n",
            "  }\n",
            "}\n",
        ),
    );
    write(root, "gfx/models/fixture.mesh", "");
    for (width, height) in [(8u32, 6u32), (6, 4), (6, 6), (8, 8)] {
        let mut header = [0; 128];
        header[..4].copy_from_slice(b"DDS ");
        header[4..8].copy_from_slice(&124u32.to_le_bytes());
        header[12..16].copy_from_slice(&height.to_le_bytes());
        header[16..20].copy_from_slice(&width.to_le_bytes());
        header[76..80].copy_from_slice(&32u32.to_le_bytes());
        header[80..84].copy_from_slice(&4u32.to_le_bytes());
        header[84..88].copy_from_slice(b"DXT1");
        write(root, &format!("gfx/{width}x{height}.dds"), header);
        if width == 8 && height == 8 {
            for path in [DDS_ORIGINAL, DDS_COPY, DDS_DISTINCT_ORIGINAL] {
                write(root, path, header);
            }
            header[123] = 1;
            write(root, DDS_DISTINCT_COPY, header);
        }
    }

    let mut data = Everything::new(None, None, None, None, root, Vec::new()).unwrap();
    data.load_all();
    data.validate_all();
    let reports = take_reports();
    let records: Vec<_> = reports
        .iter()
        .flat_map(|(report, occurrences)| occurrences.iter().map(move |locs| (report, locs)))
        .collect();
    let find = |path: &str, key: &str, text: &str| {
        records
            .iter()
            .filter(|(report, locations)| {
                locations[0].loc.pathname() == Path::new(path)
                    && report.key.to_string() == key
                    && report.msg.contains(text)
            })
            .copied()
            .collect::<Vec<_>>()
    };
    let fallback = find(ALIASES, "missing-file", "fallback video gfx/Existing.BK2 exists");
    assert_eq!(fallback.len(), 1);
    assert_eq!(fallback[0].0.confidence, Confidence::Weak);
    assert_eq!(fallback[0].0.severity, Severity::Tips);
    assert_eq!(
        fallback[0].1.iter().map(|pointer| pointer.loc.line).collect::<Vec<_>>(),
        [1, 1, 2, 3]
    );
    let texture_fallback = find(ALIASES, "missing-file", "fallback texture gfx/8x8.dds exists");
    assert_eq!(texture_fallback.len(), 1);
    assert_eq!(texture_fallback[0].0.confidence, Confidence::Weak);
    assert_eq!(
        texture_fallback[0].1.iter().map(|pointer| pointer.loc.line).collect::<Vec<_>>(),
        [8, 8, 9]
    );
    let wrong_case = find(ALIASES, "missing-file", "gfx/existing.bk2");
    assert_eq!(wrong_case.len(), 1);
    assert_eq!(wrong_case[0].0.severity, Severity::Tips);
    assert_eq!(wrong_case[0].0.confidence, Confidence::Weak);
    assert_eq!(
        wrong_case[0].0.info.as_deref(),
        Some("Use gfx/Existing.BK2 for portability to case-sensitive filesystems.")
    );
    for (line, path) in [(5, "gfx/absent.bk2"), (6, "gfx/cycle.bk2")] {
        let missing = find(ALIASES, "missing-file", path);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].1.len(), 1);
        assert_eq!(missing[0].1[0].loc.line, line);
        assert_ne!(missing[0].0.confidence, Confidence::Weak);
    }
    assert!(!find(ALIASES, "loop", "fallbacks cycle").is_empty());

    let font = find(FONT, "missing-file", "fonts/NotoSerif/NotoSansKR-Medium.otf");
    assert_eq!(font.len(), 1);
    assert_eq!(font[0].0.severity, Severity::Tips);
    assert_eq!(
        font[0].0.info.as_deref(),
        Some(
            "Existing path candidates: fonts/NotoSans/NotoSansKR-Medium.otf, fonts/NotoSerif/NotoSerifKR-Medium.otf"
        )
    );
    for path in ["gfx/8x6.dds", "gfx/6x4.dds", "gfx/8x8.dds"] {
        assert!(find(path, "image-size", "").is_empty());
    }
    let dds = find("gfx/6x6.dds", "image-size", "FourCC DXT1, 6x6");
    assert_eq!(dds.len(), 1);
    assert_eq!(dds[0].0.severity, Severity::Tips);

    let identical = find(DDS_ORIGINAL, "duplicate-item", "byte-identical DDS files");
    assert_eq!(identical.len(), 1);
    assert_eq!(identical[0].0.severity, Severity::Tips);
    let pointers = identical[0].1;
    assert_eq!(pointers.len(), 3);
    assert_eq!(pointers[1].loc.pathname(), Path::new(DDS_COPY));
    assert_eq!(pointers[2].loc.pathname(), Path::new(MESH));
    assert_eq!(pointers[2].loc.line, 3);
    let distinct = find(DDS_DISTINCT_ORIGINAL, "duplicate-item", "texture file is redefined");
    assert_eq!(distinct.len(), 1);
    assert_eq!(distinct[0].0.severity, Severity::Tips);
    assert_eq!(distinct[0].1.len(), 2);
    assert_eq!(distinct[0].1[1].loc.pathname(), Path::new(DDS_DISTINCT_COPY));
    for (line, confidence, severity, message) in [
        (3, Confidence::Weak, Severity::Tips, "#markup should be followed by a space"),
        (
            4,
            Confidence::Weak,
            Severity::Tips,
            "dynamic tooltip target has a trailing colon: [GetPlayer.GetTooltipTag],fixture_tooltip",
        ),
        (5, Confidence::Reasonable, Severity::Warning, "expected `;`"),
        (7, Confidence::Reasonable, Severity::Warning, "mismatched braces in markup"),
    ] {
        let findings = find(MARKUP, "markup", message);
        assert!(
            findings.iter().any(|(report, pointers)| {
                pointers[0].loc.line == line
                    && report.confidence == confidence
                    && report.severity == severity
            }),
            "missing expected markup diagnostic on line {line}: {findings:?}"
        );
    }
    assert!(find(MARKUP, "markup", "").iter().all(|(_, pointers)| pointers[0].loc.line != 2));

    let flow = find(PM, "modifiers", "propagation from building needs verification");
    assert_eq!(flow.len(), 3);
    let mut lines = Vec::new();
    for (report, pointers) in flow {
        assert_eq!(report.severity, Severity::Tips);
        assert_eq!(report.confidence, Confidence::Weak);
        lines.push(pointers[0].loc.line);
    }
    lines.sort_unstable();
    assert_eq!(lines, [2, 3, 4]);
    assert!(find(PM, "modifiers", "building_throughput_add").is_empty());
    let definition = find(PM, "missing-item", "modifier type definition does not exist");
    assert_eq!(definition.len(), 2);
    assert!(definition.iter().any(|(report, pointers)| {
        pointers[0].loc.line == 6
            && report.info.as_ref().unwrap().contains("known engine modifier list")
    }));
    assert!(definition.iter().any(|(report, pointers)| {
        pointers[0].loc.line == 7 && report.info.as_ref().unwrap().contains("does not recognize")
    }));

    let bom = find(VALUES, "parse-error", "U+FEFF");
    assert_eq!(bom.len(), 2);
    assert!(bom.iter().any(|(_, locs)| locs[0].loc.line == 1 && locs[0].loc.column == 1));
    assert!(bom.iter().any(|(_, locs)| locs[0].loc.line == 2 && locs[0].loc.column == 1));

    for (line, key, message, confidence) in [
        (1, "validation", "round = 1 uses an undocumented numeric argument", Confidence::Weak),
        (2, "validation", "floor expects yes or no", Confidence::Weak),
        (3, "validation", "expected yes or no", Confidence::Reasonable),
        (4, "field-missing", "has no limit", Confidence::Weak),
    ] {
        let matches = find(VALUES, key, message);
        assert_eq!(matches.len(), 1, "{message}");
        assert_eq!(matches[0].1[0].loc.line, line);
        assert_eq!(matches[0].0.confidence, confidence);
    }
}
