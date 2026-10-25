//! JCL structural language plugin (`handler = "custom"`).
//!
//! Card/statement extractor for JOB, PROC, EXEC PGM/PROC, INCLUDE MEMBER,
//! DD DSN datasets, and IKJEFT01 SYSTSIN RUN PROGRAM — no tree-sitter dependency.

mod extract;

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use extract::JclPlugin;

/// Register the JCL language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(JclPlugin));
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
    use std::path::Path;

    #[test]
    fn extracts_nightly_job_steps_db2_runner_and_datasets() {
        let source = br#"
//NIGHTLY JOB (ACCT),'NIGHTLY BILL',CLASS=A,MSGCLASS=X
//STEP1   EXEC PGM=PLIEXTR
//OUTDD   DD DSN=PROD.CUST.WORK,DISP=(NEW,CATLG),
//           SPACE=(CYL,(10,5))
//STEP2   EXEC PGM=IKJEFT01
//SYSTSIN DD *
  RUN PROGRAM(COBPOST) PLAN(COBPOST) LIB('PROD.LOAD')
/*
//INDD    DD DSN=PROD.CUST.WORK,DISP=SHR
"#;
        let result = JclPlugin
            .extract_all(Path::new("nightly.jcl"), source)
            .expect("extract");

        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "NIGHTLY" && s.symbol_type == SymbolType::Module),
            "expected NIGHTLY module, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| {
                s.name == "STEP1"
                    && s.symbol_type == SymbolType::Function
                    && s.qualified_name.as_deref() == Some("NIGHTLY::STEP1")
            }),
            "expected NIGHTLY::STEP1, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| {
                s.name == "STEP2"
                    && s.symbol_type == SymbolType::Function
                    && s.qualified_name.as_deref() == Some("NIGHTLY::STEP2")
            }),
            "expected NIGHTLY::STEP2, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP1" && r.to == "PLIEXTR" && r.relation_type == RelationType::Calls
            }),
            "expected STEP1 -> PLIEXTR, got {:?}",
            result.relations
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP2" && r.to == "COBPOST" && r.relation_type == RelationType::Calls
            }),
            "expected STEP2 -> COBPOST via IKJEFT01 SYSTSIN, got {:?}",
            result.relations
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| { s.name == "PROD.CUST.WORK" && (s.symbol_type == SymbolType::Variable) }),
            "expected dataset symbol PROD.CUST.WORK, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP1"
                    && r.to == "PROD.CUST.WORK"
                    && r.relation_type == RelationType::References
            }),
            "expected STEP1 References PROD.CUST.WORK, got {:?}",
            result.relations
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP2"
                    && r.to == "PROD.CUST.WORK"
                    && r.relation_type == RelationType::References
            }),
            "expected STEP2 References PROD.CUST.WORK, got {:?}",
            result.relations
        );
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP1"
                    && r.to == "STEP2"
                    && r.relation_type == RelationType::References
                    && r.metadata.get("kind") == Some(&serde_json::json!("dataset-pipeline"))
            }),
            "expected STEP1 -> STEP2 dataset-pipeline relation, got {:?}",
            result.relations
        );
    }

    #[test]
    fn operand_value_respects_token_boundaries() {
        let source = br#"
//BOUND  JOB
//STEP1  EXEC MYPGM=FAKE,PGM=REALPGM
"#;
        let result = JclPlugin
            .extract_all(Path::new("bound.jcl"), source)
            .expect("extract");
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "STEP1" && r.to == "REALPGM"),
            "expected STEP1 -> REALPGM, got {:?}",
            result.relations
        );
        assert!(
            !result.relations.iter().any(|r| r.to == "FAKE"),
            "must not match MYPGM=FAKE as PGM=: {:?}",
            result.relations
        );
    }

    #[test]
    fn extracts_exec_proc_and_include_member() {
        let source = br#"
//DAILY  JOB
//STEP1  EXEC PROC=BILLPROC
//       INCLUDE MEMBER=COMMON
"#;
        let result = JclPlugin
            .extract_all(Path::new("daily.jcl"), source)
            .expect("extract");
        assert!(
            result.relations.iter().any(|r| {
                r.from == "STEP1" && r.to == "BILLPROC" && r.relation_type == RelationType::Calls
            }),
            "expected STEP1 -> BILLPROC, got {:?}",
            result.relations
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "COMMON" && s.symbol_type == SymbolType::Import),
            "expected INCLUDE MEMBER=COMMON, got {:?}",
            result.symbols
        );
    }

    #[test]
    fn language_id_and_extensions() {
        assert_eq!(JclPlugin.language_id(), "jcl");
        assert_eq!(JclPlugin.file_extensions(), vec!["jcl", "proc", "cntl"]);
        assert!(JclPlugin.grammar().is_none());
    }
}
