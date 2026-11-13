use crate::block::Block;
use crate::ck3::modif::ModifKinds;
use crate::db::{Db, DbKind};
use crate::everything::Everything;
use crate::game::GameFlags;
use crate::item::{Item, ItemLoader};
use crate::modif::validate_modifs;
use crate::report::{ErrorKey, err, warn};
use crate::token::Token;
use crate::validator::Validator;

#[derive(Clone, Debug)]
pub struct SpiritualFulfillmentType {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Ck3, Item::SpiritualFulfillmentType, SpiritualFulfillmentType::add)
}

impl SpiritualFulfillmentType {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add(Item::SpiritualFulfillmentType, key, block, Box::new(Self {}));
    }
}

impl DbKind for SpiritualFulfillmentType {
    fn add_subitems(&self, _key: &Token, block: &Block, db: &mut Db) {
        for block in block.get_field_blocks("level") {
            if let Some(block) = block.get_field_block("flags") {
                for token in block.iter_values() {
                    db.add_flag(Item::FulfillmentParameter, token.clone());
                }
            }
        }
    }

    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);

        vd.field_list_items("religions", Item::Religion);
        validate_religions(key, block, data);

        let min = data
            .get_defined_string("NReligion|MIN_SPIRITUAL_FULFILLMENT")
            .and_then(Token::get_number)
            .unwrap_or(-100.0);
        let max = data
            .get_defined_string("NReligion|MAX_SPIRITUAL_FULFILLMENT")
            .and_then(Token::get_number)
            .unwrap_or(100.0);

        vd.req_field("level");
        let mut thresholds = Vec::new();
        vd.multi_field_validated_block("level", |block, data| {
            let mut vd = Validator::new(block, data);
            vd.field_numeric_range("threshold", min..=max);
            let threshold =
                block.get_field_value("threshold").and_then(Token::get_number).unwrap_or(min);
            if let Some((other, _)) =
                thresholds.iter().find(|(_, t): &&(_, f64)| (t - threshold).abs() < f64::EPSILON)
            {
                let msg = "levels must have unique thresholds";
                err(ErrorKey::DuplicateField)
                    .msg(msg)
                    .loc(block)
                    .loc_msg(*other, "other level")
                    .push();
            }
            thresholds.push((block.loc, threshold));

            vd.field_validated_block("modifier", |block, data| {
                let vd = Validator::new(block, data);
                validate_modifs(block, data, ModifKinds::Character, vd);
            });
            vd.req_field("icon");
            vd.field_item("icon", Item::File);
            vd.field_validated_list("flags", |flag, data| {
                let loca = format!("fulfillment_level_parameter_{flag}");
                data.verify_exists_implied(Item::Localization, &loca, flag);
            });
        });

        // The docs say level names are `<key>_level_<index>`, falling back to
        // `default_fulfillment_level_<index>`, where the index is the position by threshold
        for index in 0..thresholds.len() {
            let loca = format!("{key}_level_{index}");
            let fallback = format!("default_fulfillment_level_{index}");
            if data.item_exists(Item::Localization, &loca) {
                data.mark_used(Item::Localization, &loca);
            } else if data.item_exists(Item::Localization, &fallback) {
                data.mark_used(Item::Localization, &fallback);
            } else {
                data.verify_exists_implied(Item::Localization, &loca, key);
            }
        }
    }
}

/// The docs say no two types may share a religion, since that is how characters get their type,
/// and that one type should have an empty religion list as the fallback
fn validate_religions(key: &Token, block: &Block, data: &Everything) {
    let religions = block.get_field_list("religions").unwrap_or_default();
    for (other_key, other_block) in data.database.iter_key_block(Item::SpiritualFulfillmentType) {
        // Report each pair only once
        if other_key.as_str() >= key.as_str() {
            continue;
        }
        let other_religions = other_block.get_field_list("religions").unwrap_or_default();
        if religions.is_empty() && other_religions.is_empty() {
            let msg = "more than one spiritual fulfillment type without religions";
            warn(ErrorKey::Conflict).msg(msg).loc(key).loc_msg(other_key, "other type").push();
        }
        for religion in &religions {
            if let Some(other) = other_religions.iter().find(|r| r == &religion) {
                let msg =
                    format!("religion {religion} is in more than one spiritual fulfillment type");
                err(ErrorKey::Conflict).msg(msg).loc(religion).loc_msg(other, "other type").push();
            }
        }
    }
}
