use crate::block::Block;
use crate::db::{Db, DbKind};
use crate::everything::Everything;
use crate::game::GameFlags;
use crate::helpers::TigerHashSet;
use crate::item::{Item, ItemLoader};
use crate::report::{ErrorKey, tips, untidy};
use crate::token::Token;
use crate::validator::Validator;

#[derive(Clone, Debug)]
pub struct MediaAlias {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::MediaAlias, MediaAlias::add)
}

impl MediaAlias {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add_exact_dup_ok(Item::MediaAlias, key, block, Box::new(Self {}));
    }
}

impl DbKind for MediaAlias {
    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);

        let fallback = inspect_fallbacks(key, block, data);
        vd.field_validated_value("video", |_, mut value| {
            let video = value.value();
            if let Some((kind, terminal, links)) = &fallback
                && !data.fileset.exists(video.as_str())
            {
                data.fileset.mark_used(video.as_str());
                let msg = format!("file {video} does not exist; fallback {kind} {terminal} exists");
                let mut diagnostic = tips(ErrorKey::MissingFile).weak().msg(msg).loc(video);
                for link in links {
                    diagnostic = diagnostic.loc_msg(*link, "fallback alias reference");
                }
                diagnostic.loc_msg(*terminal, format!("existing fallback {kind} reference")).push();
                value.accept();
            } else {
                value.item(Item::File);
            }
        });
        vd.field_validated_value("audio", |_, mut vd| {
            vd.maybe_is("");
            vd.item(Item::Sound);
        });
        vd.field_item("texture", Item::File);
        vd.field_item("fallback", Item::MediaAlias);
    }
}

fn inspect_fallbacks<'a>(
    key: &'a Token,
    mut block: &'a Block,
    data: &'a Everything,
) -> Option<(&'static str, &'a Token, Vec<&'a Token>)> {
    let mut seen = TigerHashSet::default();
    let mut links = Vec::new();
    let mut terminal = None;
    seen.insert(key.as_str());
    while let Some(fallback) = block.get_field_value("fallback") {
        if !seen.insert(fallback.as_str()) {
            let msg = "fallbacks cycle back to the same key";
            // TODO: check if fatal
            untidy(ErrorKey::Loop).strong().msg(msg).loc(fallback).push();
            break;
        }
        let Some((_, next)) = data.get_key_block(Item::MediaAlias, fallback.as_str()) else {
            break;
        };
        block = next;
        if terminal.is_none() {
            links.push(fallback);
            // Prefer a video as evidence if this alias supplies both media paths.
            terminal = ["video", "texture"].into_iter().find_map(|kind| {
                block
                    .get_field_value(kind)
                    .filter(|file| data.fileset.exists(file.as_str()))
                    .map(|file| (kind, file))
            });
        }
    }
    terminal.map(|(kind, file)| (kind, file, links))
}
