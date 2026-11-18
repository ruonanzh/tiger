//! Persistent named scopes known to be assigned on every path through JE activation.

use crate::block::{BV, Block};
use crate::context::{ScopeContext, Temporary};
use crate::everything::Everything;
use crate::helpers::TigerHashMap;
use crate::item::Item;
use crate::scopes::Scopes;
use crate::token::Token;

pub(super) fn inherit_saved_scopes(
    block: &Block,
    root: Scopes,
    data: &Everything,
    outcome: &mut ScopeContext,
    involved: &mut ScopeContext,
) {
    let mut activation = Activation {
        root,
        data,
        saved: TigerHashMap::default(),
        engine_je: true,
        calls_remaining: 128,
    };
    activation.visit(block, Some(root));
    for (name, (scope, origin)) in activation.saved {
        outcome.define_name_token(name, scope, &origin, Temporary::No);
        involved.define_name_token(name, scope, origin, Temporary::No);
    }
}

struct Activation<'a> {
    root: Scopes,
    data: &'a Everything,
    saved: TigerHashMap<&'static str, (Scopes, Token)>,
    engine_je: bool,
    // Bound expansion even for cyclic or repeatedly nested scripted effects.
    calls_remaining: usize,
}

impl Activation<'_> {
    // A missing current scope means this path might not execute. Writes on such a path can
    // invalidate an earlier assignment but cannot establish a name for all outcomes.
    fn visit(&mut self, block: &Block, current: Option<Scopes>) {
        for field in block.iter_fields() {
            let key = field.key();
            let value = field.bv();
            let assignment = current.filter(|_| field.is_eq());
            if let Some(effect) = self.data.get_effect(key) {
                if !value.get_value().is_some_and(|token| token.lowercase_is("yes"))
                    || effect.block.source.is_some()
                    || self.calls_remaining == 0
                {
                    // The expansion or recursive invocation could clear or replace any name.
                    self.saved.clear();
                    self.engine_je = false;
                } else {
                    self.calls_remaining -= 1;
                    self.visit(&effect.block, assignment);
                }
                continue;
            }

            match value {
                BV::Value(name) if key.lowercase_is("save_scope_as") => {
                    self.forget(name);
                    if let Some(scope) = assignment.filter(|&scope| scope != Scopes::None) {
                        self.saved.insert(name.as_str(), (scope, name.clone()));
                    }
                }
                BV::Value(name)
                    if key.lowercase_is("clear_saved_scope")
                        || key.lowercase_is("save_temporary_scope_as") =>
                {
                    self.forget(name);
                }
                BV::Block(inner)
                    if key.lowercase_is("save_scope_value_as")
                        || key.lowercase_is("save_temporary_scope_value_as") =>
                {
                    if let Some(name) = inner.get_field_value("name") {
                        self.forget(name);
                    }
                }
                BV::Block(inner) if !key.lowercase_is("show_as_tooltip") => {
                    let next = if key.lowercase_is("hidden_effect")
                        || key.lowercase_is("custom_tooltip")
                        || key.lowercase_is("custom_tooltip_no_bullet")
                    {
                        assignment
                    } else {
                        current
                            .filter(|_| field.is_eq_qeq())
                            .and_then(|scope| self.enter(key, scope))
                    };
                    self.visit(inner, next);
                }
                _ => (),
            }
        }
    }

    fn forget(&mut self, name: &Token) {
        self.saved.remove(name.as_str());
        if name.is("journal_entry") {
            self.engine_je = false;
        }
    }

    fn enter(&self, key: &Token, current: Scopes) -> Option<Scopes> {
        let mut scope = current;
        for part in key.as_str().split('.') {
            scope = if part.eq_ignore_ascii_case("root") {
                self.root
            } else if part.eq_ignore_ascii_case("this") {
                current
            } else if let Some(name) = part.strip_prefix("scope:") {
                self.saved.get(name).map(|entry| entry.0).or_else(|| {
                    (self.engine_je && name == "journal_entry").then_some(Scopes::JournalEntry)
                })?
            } else if part
                .strip_prefix("cu:")
                .is_some_and(|id| self.data.item_exists(Item::Culture, id))
            {
                Scopes::Culture
            } else if part
                .strip_prefix("cd:")
                .is_some_and(|id| self.data.item_exists(Item::Country, id))
            {
                Scopes::CountryDefinition
            } else {
                // A defined country tag, a well-typed link (including capital), and an
                // iterator's element type do not establish that a live object was selected.
                return None;
            };
        }
        Some(scope)
    }
}
