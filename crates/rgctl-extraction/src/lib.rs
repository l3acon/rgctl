//! Code discovery and extraction

pub mod discovery;

pub mod extractor;
pub mod graph_builder;
pub mod manifests;
pub mod usage_detector;

pub use discovery::{DiscoveryConfig, FileDiscoverer};
pub use extractor::{ExtractionTail, Extractor, FileExtraction, SymbolPass1Prep};
pub use graph_builder::{AnnotationArgEntry, GraphBuilder};
pub use manifests::{DependencyDeclaration, extract_manifest};
