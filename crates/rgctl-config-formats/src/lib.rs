//! Configuration format plugins

pub mod json;
pub mod properties;
pub mod span_util;
pub mod toml_plugin;
pub mod xml;
pub mod yaml;

pub use json::JsonPlugin;
pub use properties::PropertiesPlugin;
pub use toml_plugin::TomlPlugin;
pub use xml::XmlPlugin;
pub use yaml::YamlPlugin;

use rgctl_plugin_api::ConfigFormatRegistrar;
use std::sync::Arc;

/// Register built-in config format plugins (yaml, json, toml, properties, xml).
pub fn register_all<R: ConfigFormatRegistrar>(registry: &mut R) {
    registry.register_config_plugin(Arc::new(YamlPlugin::new().expect("init yaml plugin")));
    registry.register_config_plugin(Arc::new(JsonPlugin::new().expect("init json plugin")));
    registry.register_config_plugin(Arc::new(TomlPlugin::new().expect("init toml plugin")));
    registry.register_config_plugin(Arc::new(
        PropertiesPlugin::new().expect("init properties plugin"),
    ));
    registry.register_config_plugin(Arc::new(XmlPlugin::new().expect("init xml plugin")));
}
