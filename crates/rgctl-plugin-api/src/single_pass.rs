//! Shared state for single-pass AST extraction (ScopeStack + KindDispatchTable).
//!
//! Language plugins migrate from multi-walk `extract_all` by driving one iterative
//! DFS that pushes/pops [`ScopeFrame`]s and dispatches via [`KindDispatchTable`].

use crate::{KindDispatchTable, ScopeFrame, ScopeKind, ScopeStack, Symbol};

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

    /// After popping a function-like frame, build [`crate::ComplexityMetrics`].
    pub fn complexity_metrics(frame: &ScopeFrame) -> crate::ComplexityMetrics {
        let c = &frame.complexity;
        crate::ComplexityMetrics {
            cyclomatic: c.cyclomatic as usize,
            cognitive: c.cognitive as usize,
            loc: c.loc as usize,
            parameters: c.param_count as usize,
            nesting_depth: c.max_nesting as usize,
            returns: c.return_count as usize,
        }
    }
}
