use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::block::{BV, Block};
use crate::context::ScopeContext;
use crate::everything::Everything;
use crate::fileset::{FileEntry, FileHandler};
use crate::helpers::{
    BANNED_NAMES, PrefixShould, TigerHashMap, dup_error, exact_dup_error, item_prefix_should,
};
use crate::item::Item;
use crate::parse::ParserMemory;
use crate::pdxfile::PdxFile;
use crate::report::{ErrorKey, err, warn};
use crate::scopes::Scopes;
use crate::script_value::{validate_non_dynamic_script_value, validate_script_value};
use crate::token::{Loc, Token};
use crate::variables::Variables;

#[derive(Debug, Default)]
pub struct ScriptValues {
    scope_overrides: TigerHashMap<&'static str, Scopes>,
    script_values: TigerHashMap<&'static str, ScriptValue>,
}

impl ScriptValues {
    fn load_item(&mut self, key: &Token, bv: &BV) {
        if crate::game::Game::is_vic3()
            && let Some(other) = self.script_values.get(key.as_str())
            && other.explicit_override
        {
            if !other.bv.equivalent(bv) {
                crate::report::tips(ErrorKey::Prefixes)
                    .msg("plain script value loads after an explicit override")
                    .info("Tiger retains the explicit override. Verify engine precedence.")
                    .loc(key)
                    .loc_msg(&other.key, "explicit override loaded here")
                    .push();
            }
            return;
        }
        if let Some(other) = self.script_values.get(key.as_str())
            && other.key.loc.kind >= key.loc.kind
        {
            if other.bv.equivalent(bv) {
                exact_dup_error(key, &other.key, "script value");
            } else {
                dup_error(key, &other.key, "script value");
            }
        }
        if BANNED_NAMES.contains(&key.as_str()) {
            let msg = "scriptedvalue has the same name as an important builtin";
            err(ErrorKey::NameConflict).strong().msg(msg).loc(key).push();
        } else {
            match item_prefix_should(Item::ScriptValue, key, |key| {
                self.script_values.get(key).map(|entry| &entry.key)
            }) {
                PrefixShould::Insert(name) => {
                    let scope_override = self.scope_overrides.get(name.as_str()).copied();
                    let explicit_override = key.split_once(':').is_some_and(|(prefix, _)| {
                        matches!(prefix.as_str(), "REPLACE" | "TRY_REPLACE" | "REPLACE_OR_CREATE")
                    });
                    let mut entry = ScriptValue::new(name, bv.clone(), scope_override);
                    entry.explicit_override = explicit_override;
                    self.script_values.insert(entry.key.as_str(), entry);
                }
                #[cfg(any(feature = "vic3", feature = "eu5"))]
                PrefixShould::Inject(name) => match bv {
                    BV::Value(value) => {
                        let msg = "cannot inject a simple value";
                        err(ErrorKey::Prefixes).msg(msg).loc(value).push();
                    }
                    BV::Block(block) => {
                        self.script_values.entry(name.as_str()).and_modify(
                            |entry| match &mut entry.bv {
                                BV::Value(value) => {
                                    let msg = "cannot inject into a simple value";
                                    err(ErrorKey::Prefixes)
                                        .msg(msg)
                                        .loc(name)
                                        .loc_msg(&*value, "into here")
                                        .push();
                                }
                                BV::Block(old_block) => {
                                    old_block.append(&mut block.clone());
                                }
                            },
                        );
                    }
                },
                PrefixShould::Ignore => (),
            }
        }
    }

    pub fn scan_variables(&self, registry: &mut Variables) {
        for item in self.script_values.values() {
            if let Some(block) = &item.bv.get_block() {
                registry.scan(block);
            }
        }
    }

    pub fn exists(&self, key: &str) -> bool {
        self.script_values.contains_key(key)
    }

    pub fn iter_keys(&self) -> impl Iterator<Item = &Token> {
        self.script_values.values().map(|item| &item.key)
    }

    pub fn validate(&self, data: &Everything) {
        for item in self.script_values.values() {
            item.validate(data);
        }
    }

    pub fn validate_call(&self, key: &Token, data: &Everything, sc: &mut ScopeContext) {
        if let Some(item) = self.script_values.get(key.as_str()) {
            item.validate_call(key, data, sc);
        }
    }

    pub fn validate_non_dynamic_call(&self, key: &Token, data: &Everything) {
        if let Some(item) = self.script_values.get(key.as_str()) {
            item.validate_non_dynamic_call(data);
        }
    }
}

impl FileHandler<Block> for ScriptValues {
    fn config(&mut self, config: &Block) {
        if let Some(block) = config.get_field_block("scope_override") {
            for (key, token) in block.iter_assignments() {
                let mut scopes = Scopes::empty();
                if token.lowercase_is("all") {
                    scopes = Scopes::all();
                } else {
                    for part in token.split('|') {
                        if let Some(scope) = Scopes::from_snake_case(part.as_str()) {
                            scopes |= scope;
                        } else {
                            let msg = format!("unknown scope type `{part}`");
                            warn(ErrorKey::Config).msg(msg).loc(part).push();
                        }
                    }
                }
                self.scope_overrides.insert(key.as_str(), scopes);
            }
        }
    }

    fn subpath(&self) -> PathBuf {
        PathBuf::from("common/script_values")
    }

    fn load_file(&self, entry: &FileEntry, parser: &ParserMemory) -> Option<Block> {
        if !entry.filename().to_string_lossy().ends_with(".txt") {
            return None;
        }

        PdxFile::read(entry, parser)
    }

    fn handle_file(&mut self, _entry: &FileEntry, block: Block) {
        for (key, bv) in block.iter_assignments_and_definitions_warn() {
            self.load_item(key, bv);
        }
    }
}

#[derive(Debug)]
pub struct ScriptValue {
    key: Token,
    explicit_override: bool,
    bv: BV,
    cache: RwLock<TigerHashMap<Loc, ScopeContext>>,
    scope_override: Option<Scopes>,
}

thread_local! {
    static IN_FLIGHT: RefCell<Vec<InferenceFrame>> = const { RefCell::new(Vec::new()) };
}

struct InferenceFrame {
    value: *const ScriptValue,
    location: Loc,
    recursive: bool,
}

struct InferenceGuard;

impl InferenceGuard {
    fn enter(value: &ScriptValue, location: Loc) -> Option<Self> {
        IN_FLIGHT.with_borrow_mut(|stack| {
            if stack
                .iter()
                .any(|frame| std::ptr::eq(frame.value, value) && frame.location == location)
            {
                // Inferences still on the stack may depend on the skipped call.
                for frame in stack {
                    frame.recursive = true;
                }
                return None;
            }
            stack.push(InferenceFrame { value, location, recursive: false });
            Some(Self)
        })
    }

    fn recursive(&self) -> bool {
        IN_FLIGHT.with_borrow(|stack| stack.last().unwrap().recursive)
    }
}

impl Drop for InferenceGuard {
    fn drop(&mut self) {
        IN_FLIGHT.with_borrow_mut(|stack| {
            stack.pop();
        });
    }
}

impl ScriptValue {
    pub fn new(key: Token, bv: BV, scope_override: Option<Scopes>) -> Self {
        Self {
            key,
            bv,
            cache: RwLock::new(TigerHashMap::default()),
            scope_override,
            explicit_override: false,
        }
    }

    fn with_inferred_context(
        &self,
        location: Loc,
        infer: impl FnOnce() -> ScopeContext,
        apply: impl FnOnce(&ScopeContext),
    ) {
        if let Some(context) = self.cache.read().unwrap().get(&location) {
            apply(context);
            return;
        }
        // Another worker may infer the same call concurrently. It can work independently using
        // its own stack, avoiding both partial cache hits and waits between recursive callers.
        let Some(guard) = InferenceGuard::enter(self, location) else {
            return;
        };
        let context = infer();
        apply(&context);
        if !guard.recursive() {
            self.cache.write().unwrap().insert(location, context);
        }
    }

    pub fn validate(&self, data: &Everything) {
        // For some reason, script values can be set to bools as well
        if let Some(token) = self.bv.get_value()
            && (token.is("yes") || token.is("no"))
        {
            return;
        }
        let mut sc = ScopeContext::new_unrooted(Scopes::all(), &self.key);
        sc.set_strict_scopes(false);
        if self.scope_override.is_some() {
            sc.set_no_warn(true);
        }
        self.validate_call(&self.key, data, &mut sc);
    }

    pub fn validate_call(&self, key: &Token, data: &Everything, sc: &mut ScopeContext) {
        self.with_inferred_context(
            key.loc,
            || {
                let mut our_sc = ScopeContext::new_unrooted(Scopes::all(), &self.key);
                our_sc.set_strict_scopes(false);
                if self.scope_override.is_some() {
                    our_sc.set_no_warn(true);
                }
                validate_script_value(&self.bv, data, &mut our_sc);
                if let Some(scopes) = self.scope_override {
                    our_sc = ScopeContext::new_unrooted(scopes, key);
                    our_sc.set_strict_scopes(false);
                }
                our_sc
            },
            |our_sc| sc.expect_compatibility(our_sc, key, data),
        );
    }

    pub fn validate_non_dynamic_call(&self, data: &Everything) {
        validate_non_dynamic_script_value(&self.bv, data);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::fileset::{FileKind, FileStage};

    use super::*;

    fn new_value(name: &str) -> ScriptValue {
        let loc = Loc::for_file(PathBuf::new(), FileStage::NoStage, FileKind::Mod, PathBuf::new());
        let token = Token::new(name, loc);
        ScriptValue::new(token.clone(), BV::Value(token), None)
    }

    #[test]
    fn overlapping_callers_infer_until_the_shared_context_is_complete() {
        let value = new_value("parallel");
        let loc = value.key.loc;
        let context = || ScopeContext::new(Scopes::None, &value.key);
        let (started, start_received) = mpsc::channel();
        let (allow_finish, finish_allowed) = mpsc::channel();
        thread::scope(|scope| {
            scope.spawn(|| {
                value.with_inferred_context(
                    loc,
                    move || {
                        started.send(()).unwrap();
                        finish_allowed.recv_timeout(Duration::from_secs(5)).unwrap();
                        context()
                    },
                    |_| {},
                );
            });

            start_received.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(value.cache.read().unwrap().is_empty());
            let mut inferred = false;
            value.with_inferred_context(
                loc,
                || {
                    inferred = true;
                    context()
                },
                |_| {},
            );
            allow_finish.send(()).unwrap();
            assert!(inferred, "the concurrent caller must infer its own context");
        });
        value.with_inferred_context(loc, || panic!("complete context should be cached"), |_| {});
    }

    #[test]
    fn recursive_calls_unwind_without_caching_incomplete_context() {
        let first = new_value("first");
        let second = new_value("second");
        let loc = first.key.loc;
        let context = || ScopeContext::new(Scopes::None, &first.key);
        let mut second_applied = false;
        first.with_inferred_context(
            loc,
            || {
                second.with_inferred_context(
                    loc,
                    || {
                        first.with_inferred_context(
                            loc,
                            || panic!("the first value is already on this thread's stack"),
                            |_| panic!("recursive call has no completed context"),
                        );
                        context()
                    },
                    |_| second_applied = true,
                );
                context()
            },
            |_| {},
        );
        assert!(second_applied, "the same location may call a different script value");
        assert!(first.cache.read().unwrap().is_empty());
        assert!(second.cache.read().unwrap().is_empty());
        first.with_inferred_context(loc, context, |_| {});
        assert!(first.cache.read().unwrap().contains_key(&loc));
    }
}
