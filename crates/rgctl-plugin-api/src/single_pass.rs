//! Shared state for single-pass AST extraction (ScopeStack + KindDispatchTable).
//!
//! Language plugins migrate from multi-walk `extract_all` by driving one iterative
//! DFS that pushes/pops [`ScopeFrame`]s and dispatches via [`KindDispatchTable`].

use crate::{
    AstHandlerTable, ComplexityMetrics, KindDispatchTable, ScopeFrame, ScopeKind, ScopeStack,
    Symbol,
};
use tree_sitter::Language;

/// Metadata key for complexity harvested during `extract_all`.
pub const COMPLEXITY_META_KEY: &str = "rgctl_complexity";

/// Read complexity attached during single-pass extraction.
pub fn complexity_from_metadata(symbol: &Symbol) -> Option<ComplexityMetrics> {
    let v = symbol.metadata.get(COMPLEXITY_META_KEY)?;
    serde_json::from_value(v.clone()).ok()
}

/// Persist complexity onto a symbol's JSON metadata.
pub fn attach_complexity(symbol: &mut Symbol, metrics: &ComplexityMetrics) {
    if let Ok(v) = serde_json::to_value(metrics) {
        if let Some(obj) = symbol.metadata.as_object_mut() {
            obj.insert(COMPLEXITY_META_KEY.to_string(), v);
        }
    }
}

/// Parse `{lang}-ast-coverage.json` `handlers` object into `(name, label)` pairs.
pub fn coverage_handler_entries(manifest_json: &str) -> Vec<(String, String)> {
    let v: serde_json::Value =
        serde_json::from_str(manifest_json).expect("ast-coverage.json parse");
    v["handlers"]
        .as_object()
        .expect("handlers object")
        .iter()
        .map(|(k, val)| (k.clone(), val.as_str().unwrap_or("Skip").to_string()))
        .collect()
}

/// Build [`AstHandlerTable`] + [`KindDispatchTable`] from a coverage manifest.
pub fn coverage_tables_from_manifest(
    grammar: &Language,
    manifest_json: &str,
) -> (AstHandlerTable, KindDispatchTable) {
    let entries = coverage_handler_entries(manifest_json);
    let handlers = AstHandlerTable::from_coverage_entries(grammar, entries.iter().map(|(n, l)| (n.as_str(), l.as_str())));
    let dispatch = KindDispatchTable::from_coverage(grammar, entries.iter().map(|(n, l)| (n.as_str(), l.as_str())));
    (handlers, dispatch)
}

/// Mutable context held for the duration of one file's single AST pass.
#[derive(Debug)]
pub struct SinglePassCtx {
    pub scopes: ScopeStack,
    pub dispatch: KindDispatchTable,
    pub symbols: Vec<Symbol>,
}

impl SinglePassCtx {
    pub fn new(dispatch: KindDispatchTable) -> Self {
        Self {
            scopes: ScopeStack::new(),
            dispatch,
            symbols: Vec::new(),
        }
    }

    /// Push a scope and optionally bind it to a just-pushed symbol.
    pub fn enter_scope(
        &mut self,
        kind: ScopeKind,
        name: Option<String>,
        qualified_prefix: String,
        start_byte: usize,
        end_byte: usize,
        bind_last_symbol: bool,
    ) {
        let depth = self.scopes.depth();
        let mut frame = ScopeFrame::new(kind, name, qualified_prefix, start_byte, end_byte, depth);
        if bind_last_symbol && !self.symbols.is_empty() {
            frame.symbol_idx = Some(self.symbols.len() - 1);
        }
        self.scopes.push(frame);
    }

    pub fn leave_scope(&mut self) -> Option<ScopeFrame> {
        self.scopes.pop()
    }

    /// After popping a function-like frame, build [`ComplexityMetrics`] and attach to symbol.
    pub fn harvest_function_complexity(&mut self, frame: &ScopeFrame) {
        if !frame.kind.is_function_like() {
            return;
        }
        let Some(idx) = frame.symbol_idx else {
            return;
        };
        let mut metrics = Self::complexity_metrics(frame);
        if metrics.cyclomatic == 0 {
            metrics.cyclomatic = 1;
        }
        if let Some(sym) = self.symbols.get_mut(idx) {
            attach_complexity(sym, &metrics);
        }
    }

    /// After popping a function-like frame, build [`ComplexityMetrics`].
    pub fn complexity_metrics(frame: &ScopeFrame) -> ComplexityMetrics {
        let c = &frame.complexity;
        ComplexityMetrics {
            cyclomatic: c.cyclomatic as usize,
            cognitive: c.cognitive as usize,
            loc: c.loc as usize,
            parameters: c.param_count as usize,
            nesting_depth: c.max_nesting as usize,
            returns: c.return_count as usize,
        }
    }
}
