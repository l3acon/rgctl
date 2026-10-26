//! PL/I structural language plugin (`handler = "custom"`).
//!
//! Line-oriented extractor for PACKAGE, nested PROC/PROCEDURE, DCL/DECLARE 1,
//! %INCLUDE, CALL, FETCH, and EXEC CICS — no tree-sitter dependency.

mod extract;

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use extract::PliPlugin;

/// Register the PL/I language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(PliPlugin));
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
    use std::path::Path;

    #[test]
    fn extracts_nested_proc_include_and_innermost_call() {
        let source = br#"
BILLING: PROC OPTIONS(MAIN);
  %INCLUDE CUSTINC;
  DCL 1 CUST_REC,
        2 CUST_ID CHAR(10);
  COMPUTE: PROC;
    CALL LOG_AUDIT(x);
  END COMPUTE;
END BILLING;
"#;
        let result = PliPlugin
            .extract_all(Path::new("billing.pli"), source)
            .expect("extract");

        assert!(
            result.symbols.iter().any(|s| {
                s.name == "BILLING"
                    && s.symbol_type == SymbolType::Function
                    && s.qualified_name.as_deref() == Some("BILLING")
            }),
            "expected BILLING function, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| {
                s.name == "COMPUTE"
                    && s.symbol_type == SymbolType::Function
                    && s.qualified_name.as_deref() == Some("BILLING::COMPUTE")
                    && s.location.start_line < s.location.end_line
            }),
            "expected BILLING::COMPUTE with body span, got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "CUSTINC" && s.symbol_type == SymbolType::Import),
            "expected CUSTINC import, got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "CUST_REC" && s.symbol_type == SymbolType::Class),
            "expected DCL 1 CUST_REC class, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "COMPUTE" && r.to == "LOG_AUDIT" && r.relation_type == RelationType::Calls
            }),
            "expected COMPUTE -> LOG_AUDIT, got {:?}",
            result.relations
        );
    }

    #[test]
    fn do_and_select_end_blocks_and_multiline_comments_do_not_pop_procedure() {
        let source = br#"
/* Multi-line header comment
   CALL FAKE_IN_COMMENT(x);
*/
BATCH_MAIN: PROC OPTIONS(MAIN);
  DO I = 1 TO 10;
    CALL STEP_ONE(I);
  END;
  /* leading inline comment */ CALL AFTER_DO_LOOP(I);
END BATCH_MAIN;
"#;
        let result = PliPlugin
            .extract_all(Path::new("batch.pli"), source)
            .expect("extract");
        assert!(
            !result.relations.iter().any(|r| r.to == "FAKE_IN_COMMENT"),
            "commented-out CALL must not be extracted: {:?}",
            result.relations
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "BATCH_MAIN" && r.to == "STEP_ONE"),
            "expected BATCH_MAIN -> STEP_ONE inside DO loop: {:?}",
            result.relations
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "BATCH_MAIN" && r.to == "AFTER_DO_LOOP"),
            "expected BATCH_MAIN -> AFTER_DO_LOOP after DO ... END;: {:?}",
            result.relations
        );
    }

    #[test]
    fn extracts_package_fetch_and_exec_cics() {
        let source = br#"
ACCTS: PACKAGE;
  LOADIT: PROC;
    FETCH TAXUTIL;
    EXEC CICS LINK PROGRAM('POSTER');
  END LOADIT;
END ACCTS;
"#;
        let result = PliPlugin
            .extract_all(Path::new("accts.pli"), source)
            .expect("extract");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "ACCTS" && s.symbol_type == SymbolType::Module),
            "expected ACCTS package module, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.to == "TAXUTIL"
                    && r.relation_type == RelationType::Calls
                    && r.metadata.get("dynamic") == Some(&serde_json::json!("true"))
            }),
            "expected dynamic FETCH to TAXUTIL, got {:?}",
            result.relations
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.to == "POSTER" && r.relation_type == RelationType::Calls),
            "expected CICS LINK to POSTER, got {:?}",
            result.relations
        );
    }

    #[test]
    fn language_id_and_extensions() {
        assert_eq!(PliPlugin.language_id(), "pli");
        assert_eq!(PliPlugin.file_extensions(), vec!["pli", "pl1", "inc"]);
        assert!(PliPlugin.grammar().is_none());
    }
}
