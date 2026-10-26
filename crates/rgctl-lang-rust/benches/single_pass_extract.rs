//! Criterion bench for Rust single-pass `extract_all`.
//!
//! Run: `cargo bench -p rgctl-lang-rust --bench single_pass_extract`

use criterion::{Criterion, criterion_group, criterion_main};
use rgctl_lang_rust::RustPlugin;
use rgctl_plugin_api::LanguagePlugin;
use std::path::Path;

const SAMPLE: &str = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Widget {
    pub id: u64,
    name: String,
}

impl Widget {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }

    pub fn rename(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        self.name = name;
    }
}

pub trait Drawable {
    fn draw(&self);
}

impl Drawable for Widget {
    fn draw(&self) {
        println!("{}", self.name);
    }
}

fn process(items: &[Widget]) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    for item in items {
        match item.id {
            0 => continue,
            n if n > 100 => {
                if item.name.starts_with('_') {
                    continue;
                }
                out.insert(n, item.name.clone());
            }
            _ => {
                out.insert(item.id, item.name.clone());
            }
        }
    }
    out
}

fn main() {
    let w = Widget::new(1, "a".into());
    w.draw();
    let _ = process(&[w]);
}
"#;

fn bench_extract_all(c: &mut Criterion) {
    let plugin = RustPlugin::new().expect("RustPlugin");
    let path = Path::new("src/widget.rs");
    let source = SAMPLE.as_bytes();

    c.bench_function("rust_extract_all_single_pass", |b| {
        b.iter(|| plugin.extract_all(path, source).expect("extract_all"));
    });
}

criterion_group!(benches, bench_extract_all);
criterion_main!(benches);
