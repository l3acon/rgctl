//! Language plugin registry and dynamic plugin loading

pub mod ingest_route;
pub mod m_disambiguate;
pub mod plugin_abi;
pub mod plugin_loader;

mod registry;

pub use ingest_route::{IngestRoute, classify_ingest_path};
pub use m_disambiguate::sniff_objc_vs_matlab;
pub use registry::{
    LanguageRegistry, RegistryStats, full_registry, set_full_registry_builder,
    set_registry_pre_init,
};
