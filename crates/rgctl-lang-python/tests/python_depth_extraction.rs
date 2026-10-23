//! Golden correctness for OpenSpec 3.2 Python AST depth.

use rgctl_lang_python::PythonPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn self_cls_annotated_enum_and_lambda_fields() {
    let source = br#"
from enum import Enum

class Status(Enum):
    PENDING = "pending"
    DONE = "done"

class Client:
    host: str = "localhost"
    timeout: int
    transform = lambda x: x

    def __init__(self):
        self.session = None

    @classmethod
    def configure(cls):
        cls.default_timeout = 30

    def refresh(self):
        self.token = "x"
"#;
    let plugin = PythonPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("client.py"), source)
        .expect("extract_all");

    let status = out
        .symbols
        .iter()
        .find(|s| s.name == "Status" && s.symbol_type == SymbolType::Class)
        .expect("Status");
    assert!(status.fields.iter().any(|f| f.name == "PENDING"));
    assert!(status.fields.iter().any(|f| f.name == "DONE"));

    let client = out
        .symbols
        .iter()
        .find(|s| s.name == "Client" && s.symbol_type == SymbolType::Class)
        .expect("Client");
    assert!(
        client.fields.iter().any(|f| f.name == "host" && f.field_type.as_deref() == Some("str")),
        "annotated host: {fields:?}",
        fields = client.fields
    );
    assert!(client.fields.iter().any(|f| f.name == "timeout"));
    assert!(client.fields.iter().any(|f| f.name == "transform"));
    assert!(client.fields.iter().any(|f| f.name == "session"));
    assert!(client.fields.iter().any(|f| f.name == "default_timeout"));
    assert!(client.fields.iter().any(|f| f.name == "token"));
}

#[test]
fn type_alias_with_as_guards_and_walrus() {
    let source = br#"
from typing import TYPE_CHECKING
import sys

type Point = tuple[int, int]

if TYPE_CHECKING:
    from models import Order
    DEFAULT_LIMIT = 100

try:
    import orjson
except ImportError:
    orjson = None

with open("f") as handle:
    pass

def parse(data):
    if (n := len(data)):
        return n
"#;
    let plugin = PythonPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("models.py"), source)
        .expect("extract_all");

    assert!(
        out.symbols.iter().any(|s| {
            s.symbol_type == SymbolType::TypeAlias && s.name == "Point"
        }),
        "type Point alias missing: {:?}",
        out.symbols
            .iter()
            .map(|s| (&s.name, &s.symbol_type))
            .collect::<Vec<_>>()
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "DEFAULT_LIMIT"
                && s.symbol_type == SymbolType::Variable
                && s.metadata.get("guard").and_then(|v| v.as_str()) == Some("typing")
        }),
        "guard binding DEFAULT_LIMIT missing: {:?}",
        out.symbols
    );
    assert!(
        out.symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Import && s.name.contains("Order")),
        "TYPE_CHECKING import Order missing"
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "handle"
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("with_as_target")
        }),
        "with-as handle missing: {:?}",
        out.symbols
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "n"
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("named_expression")
        }),
        "walrus n missing"
    );
}

#[test]
fn subscript_getitem_setitem_when_typed() {
    let source = br#"
def read_map(d: dict):
    x = d["a"]
    d["b"] = 1
"#;
    let plugin = PythonPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("map.py"), source)
        .expect("extract_all");

    let calls: Vec<_> = out
        .relations
        .iter()
        .filter(|r| r.relation_type == RelationType::Calls)
        .collect();
    assert!(
        calls.iter().any(|r| {
            r.to == "__getitem__"
                && r.to_qualified_hint.as_deref() == Some("dict.__getitem__")
        }),
        "getitem missing: {calls:?}"
    );
    assert!(
        calls.iter().any(|r| {
            r.to == "__setitem__"
                && r.to_qualified_hint.as_deref() == Some("dict.__setitem__")
        }),
        "setitem missing: {calls:?}"
    );
}
