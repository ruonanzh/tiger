use std::path::Path;

use sha2::{Digest, Sha256};

// Victoria 3 1.15.1 (Steam build 26591675). Engine controls confirmed that these
// files load with the final outer brace omitted. Each entry pins the original
// relative path, opening brace location, and SHA-256 of the entire script body.
// The loader strips the UTF-8 BOM. Normalize CRLF only when hashing so the
// parser sees the original text and the reported location stays accurate.
#[rustfmt::skip]
const STOCK_UNCLOSED_BLOCKS: &[(&str, u32, u32, &str)] = &[
    ("common/dna_data/00_bal_gangadhar_tilak.txt", 4, 27, "6d82d55b7ddd5741e76339016673de852f9347ea4b66711c5829ac79a08e3ffc"),
    ("common/dna_data/00_iwasaki_yataro.txt", 5, 22, "c098126a492b35eba23e975ed637317f62f12a8a364a53cfd41b71655e35cea5"),
    ("common/dna_data/00_mahatma_gandhi.txt", 4, 22, "9bdc94963532e3952928f980c43287c1e4d67b0b137ee702df20bde716d5b85f"),
    ("common/dna_data/00_rani_lakshmibai.txt", 4, 23, "9bfd7721ab7978dcaa89cc5b0ed8bd1271dcc3a30e47e12e767fcc6dab6368c1"),
    ("common/dna_data/00_sarojni_naidu.txt", 4, 21, "16009854fc355e187fc3553631fcced78f3c65da4c8d1ffe210dfa008d738a0b"),
    ("common/dna_data/00_zhou_xuexi.txt", 4, 18, "887b6096feb558a6bf7fc3bbc5470432426657e12eb71b5de012fd8ea7849e57"),
    ("gfx/models/portraits/attachments/female_headgear/japanese_common/female_headgear_japanese_common_01/female_headgear_japanese_common_01.asset", 18, 10, "98972ec7188ae8fe77e9b017b5b1eaf935b0c4c5392f77546670abf74686b52a"),
    ("gfx/models/portraits/attachments/female_headgear/japanese_common/female_headgear_japanese_common_02/female_headgear_japanese_common_02.asset", 18, 10, "8ca627d15baed96f52100e2c8b3c7aed6d12454de8475aa01e08b66580f8fbfa"),
    ("gfx/models/portraits/attachments/female_headgear/japanese_common/female_headgear_japanese_common_03/female_headgear_japanese_common_03.asset", 18, 10, "10eea503ceb4c08d2366d247eac9db7731141e6f88f184d39116c772ef905eb5"),
    ("gfx/models/portraits/attachments/female_headgear/japanese_common/female_headgear_japanese_common_04/female_headgear_japanese_common_04.asset", 19, 10, "87e6b402fdaa18c3ba38a0b45b7ab2c73866d472910775e1dc8406629f0a16d1"),
    ("gfx/models/portraits/attachments/male_headgear/european_military/male_headgear_polish_military_02/male_headgear_polish_military_02.asset", 18, 10, "4bc5b20cb3b34bd63109975917b179205bbfcc06206b422585c46ef818b9092d"),
    ("gfx/models/portraits/attachments/male_headgear/japanese_common/male_headgear_japanese_common_01/male_headgear_japanese_common_01.asset", 18, 10, "639dfeb98b1dd6d24d412f88cc57cf6532b0d9876eedfcd47c820530b3bf5c3c"),
    ("gfx/models/portraits/attachments/male_headgear/japanese_common/male_headgear_japanese_common_02/male_headgear_japanese_common_02.asset", 20, 10, "e44b222bd5e44e8f33718f2e90b1c85679fbdcac97b3d68f1e890c5eb6932348"),
    ("gfx/models/portraits/attachments/male_headgear/japanese_common/male_headgear_japanese_common_03/male_headgear_japanese_common_03.asset", 20, 10, "424210f5d8b27601babd8cf96c99b02f0cb20e8442a55dd69101ae387c62afa1"),
    ("gfx/models/portraits/attachments/male_headgear/japanese_common/male_headgear_japanese_common_04/male_headgear_japanese_common_04.asset", 20, 10, "a129f2dcecbfb2480bbcf2b985664ba2ed8c08d382b31b2da8a40b4cda1e8b41"),
    ("gfx/models/portraits/attachments/male_headgear/japanese_common/male_headgear_japanese_eboshi_01/male_headgear_japanese_eboshi_01.asset", 20, 10, "986a9d93f78f83f0a15ca4d9249032fbf29cea1d887c373b94aef90a3290dd87"),
];

pub(super) fn opening_position(path: &Path, content: &str) -> Option<(u32, u32)> {
    let &(_, line, column, digest) =
        STOCK_UNCLOSED_BLOCKS.iter().find(|entry| path == Path::new(entry.0))?;
    matches_digest(content, digest).then_some((line, column))
}

fn matches_digest(content: &str, digest: &str) -> bool {
    let normalized = content.replace("\r\n", "\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes())) == digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_digest_allows_line_endings_but_pins_the_entire_body() {
        let original = "one = {\n    two = { yes }\n";
        let digest = format!("{:x}", Sha256::digest(original.as_bytes()));
        assert!(matches_digest(original, &digest));
        assert!(matches_digest(&original.replace('\n', "\r\n"), &digest));
        assert!(!matches_digest("one = {\n    two = { yes \n}\n", &digest));
        assert!(!matches_digest("one = {\n    two = { yes \n", &digest));
        assert!(!matches_digest("one = {\n    two = { yes }\n}\n", &digest));
    }

    #[test]
    fn eof_recovery_only_warns_at_the_approved_opening_brace() {
        use crate::fileset::{FileEntry, FileKind, FileStage};
        use crate::parse::ParserMemory;
        use crate::report::{ErrorKey, Severity, take_reports};

        let cases = [
            ("known_opening.txt", Some((1, 7)), Severity::Warning),
            ("other_opening.txt", Some((2, 7)), Severity::Error),
            ("unrecognized_file.txt", None, Severity::Error),
        ];
        for (file, opening, _) in cases {
            let entry = FileEntry::new(file.into(), FileStage::NoStage, FileKind::Mod, file.into());
            super::super::parse_pdx(
                &entry,
                "one = {\n    two = { yes }\n",
                &ParserMemory::default(),
                opening,
            );
        }
        let reports = take_reports();
        for (file, _, severity) in cases {
            let mut found = reports.iter().flat_map(|(report, locations)| {
                locations.iter().filter_map(move |pointers| {
                    let loc = pointers[0].loc;
                    (report.key == ErrorKey::BraceError && loc.pathname() == Path::new(file))
                        .then_some((report, loc))
                })
            });
            let (report, loc) = found.next().expect("missing brace diagnostic");
            assert_eq!(report.severity, severity);
            assert_eq!(report.msg, "opening { was never closed");
            assert_eq!((loc.line, loc.column), (1, 7));
            assert!(found.next().is_none());
        }
    }
}
