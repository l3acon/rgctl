//! Code discovery and extraction

pub mod discovery;

pub mod extractor;
pub mod facts_cache;
pub mod graph_builder;
pub mod manifests;
pub mod usage_detector;

pub use discovery::{DiscoveryConfig, FileDiscoverer};
pub use extractor::{ExtractionTail, Extractor, FileExtraction, SymbolPass1Prep};
pub use facts_cache::{
    FACTS_CACHE_REL, FactsCache, FactsKey, facts_key_for, facts_key_for_hash,
};
pub use graph_builder::{AnnotationArgEntry, GraphBuilder};
pub use manifests::{DependencyDeclaration, extract_declarations, extract_manifest};
