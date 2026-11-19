use std::collections::BTreeMap;
use std::mem::take;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::block::Block;
use crate::context::ScopeContext;
use crate::db::{Db, DbKind};
use crate::everything::Everything;
use crate::fileset::FileKind;
use crate::game::GameFlags;
use crate::item::{Item, ItemLoader, LoadAsFile, Recursive};
use crate::pdxfile::PdxEncoding;
use crate::report::{ErrorKey, ErrorLoc, LogReportPointers, PointedMessage, tips, warn};
use crate::scopes::Scopes;
use crate::token::Token;
use crate::tooltipped::Tooltipped;
use crate::validator::{Validator, ValueValidator};

#[derive(Debug, Default)]
pub(crate) struct OptionalThemeAssets(Mutex<BTreeMap<(String, FileKind), LogReportPointers>>);

impl OptionalThemeAssets {
    fn record(&self, dlc: &Token, kind: Item, value: &Token) {
        let location = PointedMessage {
            loc: value.loc,
            length: value.loc_length(),
            msg: Some(format!("{kind} {value}")),
        };
        self.0.lock().unwrap().entry((dlc.to_string(), value.loc.kind)).or_default().push(location);
    }

    pub(crate) fn report(&self) {
        for ((dlc, _), mut locations) in take(&mut *self.0.lock().unwrap()) {
            locations.sort_by_key(|pointer| {
                (pointer.loc.fullpath(), pointer.loc.line, pointer.loc.column)
            });
            tips(ErrorKey::OptionalDlc)
                .msg(format!(
                    "Theme assets could not be checked because DLC {dlc} is not installed"
                ))
                .pointers(locations)
                .push();
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::Theme, Theme::add)
}

impl Theme {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add(Item::Theme, key, block, Box::new(Self {}));
    }

    fn absent_dlc<'a>(block: &'a Block, data: &Everything) -> Option<&'a Token> {
        let mut fields = block.iter_fields().filter(|field| field.key().is("dlc"));
        let field = fields.next()?;
        if !field.is_eq() || fields.next().is_some() {
            return None;
        }
        let dlc = field.bv().get_value()?;
        let (key, metadata) = data.database.get_key_block(Item::Dlc, dlc.as_str())?;
        (key.loc.kind == FileKind::Vanilla
            && metadata.get_field_bool("theme_provider") == Some(true)
            && data.fileset.vic3_dlc_payload_absent(dlc.as_str()))
        .then_some(dlc)
    }

    fn check_dlc_file(
        mut value: ValueValidator,
        absent_dlc: Option<&Token>,
        is_payload_path: impl FnOnce(&str, &str) -> bool,
    ) {
        let token = value.value();
        if let Some(dlc) = absent_dlc
            && is_payload_path(token.as_str(), dlc.as_str())
            && !value.data().item_exists(Item::File, token.as_str())
        {
            value.data().optional_theme_assets.record(dlc, Item::File, token);
            value.accept();
        } else {
            value.item(Item::File);
        }
    }

    fn is_payload_name(name: &str) -> bool {
        !name.is_empty() && name.bytes().all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
    }

    fn check_asset(mut value: ValueValidator, kind: Item, absent_dlc: Option<&Token>) {
        let token = value.value();
        if let Some(dlc) = absent_dlc
            && !token.as_str().is_empty()
            && !token.as_str().contains(['.', ':', '/', '\\'])
            && !token.as_str().chars().any(char::is_whitespace)
            && !value.data().item_exists(kind, token.as_str())
        {
            value.data().optional_theme_assets.record(dlc, kind, token);
            value.accept();
        } else {
            value.item(kind);
        }
    }
}

impl DbKind for Theme {
    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);

        data.verify_exists(Item::Localization, key);
        let loca = format!("{key}_desc");
        data.verify_exists_implied(Item::Localization, &loca, key);

        let absent_dlc = Self::absent_dlc(block, data);
        vd.req_field("category");
        vd.field_choice("category", THEME_CATEGORIES);
        if let Some(category) = block.get_field_value("category") {
            data.verify_exists(Item::Localization, category);
            let loca = format!("{category}_desc");
            data.verify_exists_implied(Item::Localization, &loca, category);

            vd.advice_field(
                "map_textures",
                "docs say map_textures, but it's papermap_textures_file",
            );
            if category.is("papermap_theme") {
                vd.field_validated_value("papermap_textures_file", |_, value| {
                    Self::check_dlc_file(value, absent_dlc, |path, _| {
                        key.as_str().strip_prefix("papermap_").is_some_and(|name| {
                            Self::is_payload_name(name)
                                && path == format!("gfx/map/textures/maptextures_{name}.settings")
                        })
                    });
                });
            } else {
                vd.ban_field("papermap_textures_file", || "category papermap_theme");
            }
        }

        vd.field_trigger_rooted("selectable", Tooltipped::No, Scopes::None);

        vd.advice_field("skin", "docs say skin, but it's ui_skin");
        vd.field_validated_value("ui_skin", |_, mut value| {
            if value.value().is("default") {
                value.accept();
            } else {
                Self::check_asset(value, Item::Skin, absent_dlc);
            }
        });

        vd.advice_field("theme_object", "docs say theme_object, but it's map_object");
        vd.multi_field_validated_block("map_object", |block, data| {
            let mut vd = Validator::new(block, data);
            vd.req_field_one_of(&["pdxmesh", "entity"]);
            vd.field_validated_value("pdxmesh", |_, value| {
                Self::check_asset(value, Item::Pdxmesh, absent_dlc);
            });
            vd.field_validated_value("entity", |_, value| {
                Self::check_asset(value, Item::Entity, absent_dlc);
            });
            vd.req_field("locator");
            vd.field_value("locator"); // TODO
        });

        vd.field_item("dlc", Item::Dlc);
        vd.field_value("bulk_selection_group");
        let mut sc = ScopeContext::new(Scopes::Country, key);
        vd.field_script_value_no_breakdown("dynamic_weight", &mut sc);

        // undocumented

        vd.field_validated_value("video", |_, value| {
            Self::check_dlc_file(value, absent_dlc, |path, dlc| {
                block.field_value_is("category", "main_menu_image_theme")
                    && path
                        .strip_prefix(&format!("dlc/{dlc}_"))
                        .and_then(|rest| {
                            rest.strip_suffix(
                                "/gfx/interface/illustrations/frontend/menu_video.bk2",
                            )
                        })
                        .is_some_and(Self::is_payload_name)
            });
        });
    }
}

#[derive(Clone, Debug)]
pub struct Skin {}

inventory::submit! {
    ItemLoader::Full(GameFlags::Vic3, Item::Skin, PdxEncoding::Utf8Bom, ".skin", LoadAsFile::Yes, Recursive::No, Skin::add)
}

impl Skin {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        if let Some(name) = block.get_field_value("name") {
            db.add(Item::Skin, name.clone(), block, Box::new(Self {}));
        } else {
            let msg = "skin file with no name field";
            warn(ErrorKey::FieldMissing).msg(msg).loc(key).push();
        }
    }
}

impl DbKind for Skin {
    fn validate(&self, _key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);

        vd.field_value("name");
        vd.field_item("folder", Item::Directory);

        if let Some(folder) = block.get_field_value("folder") {
            for entry in data.fileset.get_files_under(&PathBuf::from(folder.as_str())) {
                let path = entry.path().to_string_lossy();
                // This slice should never fail, because of how get_files_under() works
                let base_path = format!("gfx/interface{}", &path[folder.as_str().len()..]);
                if !data.item_exists(Item::File, &base_path) {
                    let msg = format!("file {base_path} does not exist");
                    let info = "every skin file must override an existing interface file";
                    warn(ErrorKey::ExtraFile).msg(msg).info(info).loc(entry).push();
                }
            }
        }
    }
}

const THEME_CATEGORIES: &[&str] = &[
    "ui_skin_theme",
    "papermap_theme",
    "table_top_theme",
    "table_asset_1_theme",
    "table_asset_2_theme",
    "table_asset_3_theme",
    "table_asset_4_theme",
    "table_asset_cloth_theme",
    "building_sets_themes",
    // undocumented
    "main_menu_image_theme",
];
