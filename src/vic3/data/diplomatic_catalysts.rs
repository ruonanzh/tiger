use crate::block::Block;
use crate::context::ScopeContext;
use crate::db::{Db, DbKind};
use crate::everything::Everything;
use crate::game::GameFlags;
use crate::item::{Item, ItemLoader};
use crate::scopes::Scopes;
use crate::script_value::validate_script_value_no_breakdown;
use crate::token::Token;
use crate::tooltipped::Tooltipped;
use crate::validate::validate_duration;
use crate::validator::Validator;
use crate::vic3::tables::misc::LOBBY_FORMATION_REASON;

#[derive(Clone, Debug)]
pub struct DiplomaticCatalystCategory {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::DiplomaticCatalystCategory, DiplomaticCatalystCategory::add)
}

impl DiplomaticCatalystCategory {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add(Item::DiplomaticCatalystCategory, key, block, Box::new(Self {}));
    }
}

impl DbKind for DiplomaticCatalystCategory {
    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);
        vd.req_field("duration");

        // TODO: verify if catalyst categories allow script values at all
        let mut sc = ScopeContext::new(Scopes::Country, key);
        vd.field_validated_block_sc("duration", &mut sc, validate_duration);
        // TODO: validate "cannot be longer than duration"
        vd.field_validated_block_sc("catalyst_effect_cooldown", &mut sc, validate_duration);

        vd.field_choice("lobby_formation_reason", LOBBY_FORMATION_REASON);
    }
}

#[derive(Clone, Debug)]
pub struct DiplomaticCatalyst {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::DiplomaticCatalyst, DiplomaticCatalyst::add)
}

impl DiplomaticCatalyst {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add(Item::DiplomaticCatalyst, key, block, Box::new(Self {}));
    }
}

impl DbKind for DiplomaticCatalyst {
    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);

        data.verify_exists(Item::Localization, key);
        let loca = format!("{key}_desc");
        data.verify_exists_implied(Item::Localization, &loca, key);

        vd.req_field("category");
        vd.field_item("category", Item::DiplomaticCatalystCategory);
        // These engine-created catalysts supply the grievance before and after the change.
        let has_strait_grievance = [
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_TOLL_HIGH",
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_TOLL_REDUCED",
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_NO_MILITARY",
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_CLOSED",
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_OPENED",
            "NDiplomacy|DIPLOMATIC_CATALYST_TYPE_STRAIT_PARTIALLY_OPENED",
        ]
        .iter()
        .any(|define| data.get_defined_string(define).is_some_and(|catalyst| catalyst == key));
        let catalyst_context = || {
            let mut sc = ScopeContext::new(Scopes::Country, key);
            sc.define_name("target_country", Scopes::Country, key);
            if has_strait_grievance {
                sc.define_name("value_before", Scopes::Value, key);
                sc.define_name("value_after", Scopes::Value, key);
            }
            sc
        };

        vd.field_validated_block("political_lobby_creation", |block, data| {
            let mut vd = Validator::new(block, data);
            let mut sc = catalyst_context();
            sc.define_name("new_diplomatic_relation", Scopes::Bool, key);

            vd.field_trigger("trigger", Tooltipped::No, &mut sc);
            vd.unknown_fields(|key, bv| {
                data.verify_exists(Item::PoliticalLobby, key);
                validate_script_value_no_breakdown(bv, data, &mut sc);
            });
        });

        vd.multi_field_validated_block("ai_country_goal_recalculation", |block, data| {
            let mut vd = Validator::new(block, data);
            let mut sc = catalyst_context();
            sc.define_name("new_diplomatic_relation", Scopes::Bool, key);
            vd.field_choice(
                "type",
                &[
                    "all",
                    "not_hostile", // undocumented
                    "more_friendly",
                    "more_hostile",
                    "only_friendly",
                    "only_hostile",
                    "only_neutral",
                ],
            );
            vd.field_trigger("trigger", Tooltipped::No, &mut sc);
            vd.field_script_value_no_breakdown("chance", &mut sc);
        });

        vd.field_effect_builder("effect", Tooltipped::No, |_| catalyst_context());
    }
}
