use rgctl_plugin_api::*;
use std::path::Path;

pub struct SwiftPlugin;

impl SwiftPlugin {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl LanguagePlugin for SwiftPlugin {
    fn language_id(&self) -> &str {
        "swift"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["swift"]
    }

    fn grammar(&self) -> Option<tree_sitter::Language> {
        Some(tree_sitter_swift::LANGUAGE.into())
    }

    fn extract_all(&self, file_path: &Path, source: &[u8]) -> Result<ExtractAllResult> {
        crate::single_pass::extract_all_single_pass(self, file_path, source)
    }

    fn extract_symbols(&self, _: &Path, _: &[u8]) -> Result<Vec<Symbol>> {
        unimplemented!("use extract_all")
    }

    fn extract_relations(&self, _: &Path, _: &[u8], _: &[Symbol]) -> Result<Vec<Relation>> {
        unimplemented!("use extract_all")
    }

    fn calculate_complexity(&self, symbol: &Symbol, _: &[u8]) -> Result<Option<ComplexityMetrics>> {
        Ok(complexity_from_metadata(symbol))
    }

    fn declared_ast_handlers(&self) -> Option<&AstHandlerTable> {
        Some(crate::single_pass::swift_ast_handlers())
    }
}
