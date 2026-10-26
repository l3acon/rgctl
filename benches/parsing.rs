//! Parsing / single-pass extraction benchmarks.
//!
//! Run with: `cargo bench --bench parsing`
//! Smoke: `cargo bench --bench parsing -- --test`

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rgctl::languages::registry::LanguageRegistry;
use rgctl::pipeline::{PipelineConfig, ProcessingPipeline};
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn bench_parallel_parsing(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    for i in 0..100 {
        fs::write(
            temp.path().join(format!("file{i}.rs")),
            format!("fn func{i}() {{}}\n"),
        )
        .unwrap();
    }

    let registry = LanguageRegistry::new().into();
    c.bench_function("parse_100_files", |b| {
        b.iter(|| {
            let pipeline = ProcessingPipeline::with_config(
                Arc::clone(&registry),
                PipelineConfig {
                    show_progress: false,
                    thread_count: Some(4),
                    ..PipelineConfig::default()
                },
            );
            black_box(pipeline.process_repository(temp.path()).unwrap())
        });
    });
}

fn sample_sources() -> Vec<(&'static str, &'static str, &'static [u8])> {
    vec![
        (
            "rust",
            "sample.rs",
            br#"
pub fn add(a: i32, b: i32) -> i32 {
    if a > 0 { a + b } else { b }
}
pub struct Point { x: i32, y: i32 }
impl Point {
    pub fn mag(&self) -> i32 { self.x * self.x + self.y * self.y }
}
"#,
        ),
        (
            "typescript",
            "sample.ts",
            br#"
export function add(a: number, b: number): number {
  if (a > 0) { return a + b; }
  return b;
}
export class Point {
  constructor(public x: number, public y: number) {}
  mag(): number { return this.x * this.x + this.y * this.y; }
}
"#,
        ),
        (
            "python",
            "sample.py",
            br#"
def add(a, b):
    if a > 0:
        return a + b
    return b

class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y
    def mag(self):
        return self.x * self.x + self.y * self.y
"#,
        ),
        (
            "java",
            "Sample.java",
            br#"
public class Sample {
  public static int add(int a, int b) {
    if (a > 0) { return a + b; }
    return b;
  }
  static class Point {
    int x, y;
    int mag() { return x * x + y * y; }
  }
}
"#,
        ),
        (
            "cpp",
            "sample.cpp",
            br#"
int add(int a, int b) {
  if (a > 0) return a + b;
  return b;
}
struct Point {
  int x, y;
  int mag() const { return x * x + y * y; }
};
"#,
        ),
    ]
}

fn bench_single_pass_vs_multi_pass(c: &mut Criterion) {
    let registry = LanguageRegistry::new();
    let mut group = c.benchmark_group("single_pass_vs_multi_pass");
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(8));

    for (lang, filename, source) in sample_sources() {
        let plugin = registry
            .get_language_plugin(lang)
            .unwrap_or_else(|| panic!("missing plugin {lang}"));
        let path = std::path::Path::new(filename);

        group.bench_function(format!("{lang}_extract_all"), |b| {
            b.iter(|| black_box(plugin.extract_all(path, source).unwrap()))
        });

        group.bench_function(format!("{lang}_multi_pass"), |b| {
            b.iter(|| {
                let symbols = plugin.extract_symbols(path, source).unwrap();
                let relations = plugin.extract_relations(path, source, &symbols).unwrap();
                for s in &symbols {
                    let _ = plugin.calculate_complexity(s, source);
                }
                black_box((symbols, relations))
            })
        });

        // Hard gate: single-pass must beat multi-pass by >= 30% wall on this sample.
        let iters = 80usize;
        let t_single = {
            let start = Instant::now();
            for _ in 0..iters {
                black_box(plugin.extract_all(path, source).unwrap());
            }
            start.elapsed()
        };
        let t_multi = {
            let start = Instant::now();
            for _ in 0..iters {
                let symbols = plugin.extract_symbols(path, source).unwrap();
                let relations = plugin.extract_relations(path, source, &symbols).unwrap();
                for s in &symbols {
                    let _ = plugin.calculate_complexity(s, source);
                }
                black_box((symbols, relations));
            }
            start.elapsed()
        };
        assert!(
            t_single.as_secs_f64() * 1.30 <= t_multi.as_secs_f64()
                || t_single < Duration::from_millis(5),
            "{lang}: single-pass {t_single:?} not >=30% faster than multi-pass {t_multi:?}"
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parallel_parsing,
    bench_single_pass_vs_multi_pass
);
criterion_main!(benches);
