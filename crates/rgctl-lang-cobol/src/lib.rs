//! COBOL structural language plugin (`handler = "custom"`).
//!
//! Line-oriented extractor for PROGRAM-ID, paragraphs/sections, COPY, PERFORM,
//! CALL, and EXEC CICS LINK/XCTL — no tree-sitter dependency.

mod extract;

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use extract::CobolPlugin;

/// Register the COBOL language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(CobolPlugin));
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
    use std::path::Path;

    #[test]
    fn extracts_program_paragraphs_copy_perform_and_call() {
        let source = br#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTBILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  CUST-REC.
           05 CUST-ID PIC X(10).
       PROCEDURE DIVISION.
       MAIN-LOGIC.
           COPY CUSTREC.
           PERFORM CALC-TAX
           CALL 'TAXUTIL'
           STOP RUN.
       CALC-TAX.
           EXIT.
"#;
        let result = CobolPlugin
            .extract_all(Path::new("custbill.cbl"), source)
            .expect("extract");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "CUSTBILL" && s.symbol_type == SymbolType::Module),
            "expected CUSTBILL module, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| {
                s.name == "MAIN-LOGIC"
                    && s.symbol_type == SymbolType::Function
                    && s.qualified_name.as_deref() == Some("CUSTBILL::MAIN-LOGIC")
                    && s.location.start_line < s.location.end_line
            }),
            "expected CUSTBILL::MAIN-LOGIC with multi-line span, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| {
                s.name == "CALC-TAX" && s.qualified_name.as_deref() == Some("CUSTBILL::CALC-TAX")
            }),
            "expected CUSTBILL::CALC-TAX, got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| { s.name == "CUSTREC" && s.symbol_type == SymbolType::Import }),
            "expected CUSTREC import, got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| { s.name == "CUST-REC" && s.symbol_type == SymbolType::Class }),
            "expected 01 CUST-REC class, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "MAIN-LOGIC"
                    && r.to == "CALC-TAX"
                    && r.relation_type == RelationType::Calls
            }),
            "expected MAIN-LOGIC -> CALC-TAX, got {:?}",
            result.relations
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "MAIN-LOGIC"
                    && r.to == "TAXUTIL"
                    && r.relation_type == RelationType::Calls
            }),
            "expected MAIN-LOGIC -> TAXUTIL, got {:?}",
            result.relations
        );
    }

    #[test]
    fn language_id_and_extensions() {
        assert_eq!(CobolPlugin.language_id(), "cobol");
        assert_eq!(CobolPlugin.file_extensions(), vec!["cbl", "cob", "cpy"]);
        assert!(CobolPlugin.grammar().is_none());
    }

    #[test]
    fn extracts_exec_cics_link_single_and_multiline_and_sql_include() {
        let source = br#"
       PROGRAM-ID. DRIVER.
       PROCEDURE DIVISION.
       ENTRY-POINT.
           EXEC SQL INCLUDE SQLCA END-EXEC.
           EXEC CICS
               LINK PROGRAM('HELPER')
               COMMAREA(WS-COMM)
           END-EXEC
           GOBACK.
"#;
        let result = CobolPlugin
            .extract_all(Path::new("driver.cbl"), source)
            .expect("extract");
        assert!(
            result.relations.iter().any(|r| {
                r.from == "ENTRY-POINT"
                    && r.to == "HELPER"
                    && r.relation_type == RelationType::Calls
            }),
            "expected CALL to HELPER via multi-line CICS LINK, got {:?}",
            result.relations
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "SQLCA" && s.symbol_type == SymbolType::Import),
            "expected SQLCA import via EXEC SQL INCLUDE, got {:?}",
            result.symbols
        );
    }

    #[test]
    fn handles_free_format_4_space_indent_and_utf8_boundaries() {
        let source = "    IDENTIFICATION DIVISION.\n    PROGRAM-ID. FREEFMT.\n    DATA DIVISION.\n    WORKING-STORAGE SECTION.\n    01 CUST-REC.\n       05 NOTES PIC X(80) VALUE 'ÄÖÜß€日本語テスト________________________________________________________________'.\n    PROCEDURE DIVISION.\n    MAIN-PARA.\n        CALL 'SUBPROG'.\n";
        let result = CobolPlugin
            .extract_all(Path::new("freefmt.cbl"), source.as_bytes())
            .expect("extract");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "CUST-REC" && s.symbol_type == SymbolType::Class),
            "expected 4-space indented 01 CUST-REC, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "MAIN-PARA" && r.to == "SUBPROG"),
            "expected MAIN-PARA -> SUBPROG call, got {:?}",
            result.relations
        );
    }
}
