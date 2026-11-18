use crate::block::Block;
use crate::db::Db;
use crate::game::GameFlags;
use crate::item::{Item, ItemLoader};
use crate::token::Token;

inventory::submit! {
    ItemLoader::Normal(GameFlags::Vic3, Item::RegionalExilePool, add_pool)
}

fn add_pool(db: &mut Db, key: Token, _block: Block) {
    db.add_flag(Item::RegionalExilePool, key);
}
