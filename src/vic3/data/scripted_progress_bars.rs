use crate::block::Block;
use crate::context::ScopeContext;
use crate::db::{Db, DbKind};
use crate::desc::validate_desc;
use crate::everything::Everything;
use crate::game::GameFlags;
use crate::item::{Item, ItemLoader};
use crate::scopes::Scopes;
use crate::token::Token;
use crate::validator::Validator;

#[derive(Clone, Debug)]
pub struct ScriptedProgressBar {}

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::ScriptedProgressBar, ScriptedProgressBar::add)
}

impl ScriptedProgressBar {
    pub fn add(db: &mut Db, key: Token, block: Block) {
        db.add(Item::ScriptedProgressBar, key, block, Box::new(Self {}));
    }
}

const BAR_TYPES: &[&str] = &[
    "default",
    "default_green",
    "default_bad",
    "double_sided_gold",
    "double_sided_bad",
    // undocumented
    "double_sided_two_spains",
];

impl DbKind for ScriptedProgressBar {
    fn validate(&self, key: &Token, block: &Block, data: &Everything) {
        let mut vd = Validator::new(block, data);
        let mut sc = ScopeContext::new(Scopes::JournalEntry, key);

        vd.req_field("name");
        vd.field_validated_sc("name", &mut sc, validate_desc);
        vd.req_field("desc");
        vd.field_validated_sc("desc", &mut sc, validate_desc);
        vd.field_validated_sc("second_desc", &mut sc, validate_desc);

        vd.field_bool("is_inverted");

        vd.field_numeric("start_value");
        vd.field_numeric("min_value");
        vd.field_numeric("max_value");

        vd.field_list_numeric_exactly("color", 4);
        let mut custom_look = block.get_field("color").is_some();
        custom_look |= vd.field_bool("marker");
        for field in [
            "progress_texture",
            "background_texture",
            "glow_texture",
            "glow_line_texture",
            "left_icon",
            "right_icon",
        ] {
            custom_look |= vd.field_item(field, Item::File);
        }

        if custom_look {
            vd.exclusive_fields(BAR_TYPES);
        } else {
            vd.req_field_one_of(BAR_TYPES);
        }
        for field in BAR_TYPES {
            vd.field_bool(field);
        }

        for field in &["weekly_progress", "monthly_progress", "yearly_progress"] {
            vd.field_scripted_progress(field);
        }
    }
}
