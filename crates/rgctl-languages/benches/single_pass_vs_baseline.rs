//! Single-pass `extract_all` vs multipass API baseline (`extract_symbols` + `extract_relations`).
//!
//! The multipass baseline intentionally uses the public two-call path (two parses + two walks)
//! that callers hit when not using `extract_all`. After migration, `extract_all` should be
//! clearly faster (target ≥30% from OpenSpec 2.6).
//!
//! Run: `cargo bench -p rgctl-languages --bench single_pass_vs_baseline`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rgctl_languages::default_registry;
use rgctl_plugin_api::LanguagePlugin;
use std::path::Path;
use std::time::Duration;

struct Sample {
    lang: &'static str,
    path: &'static str,
    source: &'static [u8],
}

const SAMPLES: &[Sample] = &[
    Sample {
        lang: "rust",
        path: "src/lib.rs",
        source: br#"
pub struct Foo { pub x: i32 }
impl Foo {
    pub fn new(x: i32) -> Self { Self { x } }
    pub fn bump(&mut self) { if self.x > 0 { self.x += 1; } }
}
fn main() { let mut f = Foo::new(1); f.bump(); }
"#,
    },
    Sample {
        lang: "python",
        path: "mod.py",
        source: br#"
class Foo:
    def __init__(self, x):
        self.x = x
    def bump(self):
        if self.x > 0:
            self.x += 1
def main():
    f = Foo(1)
    f.bump()
"#,
    },
    Sample {
        lang: "javascript",
        path: "mod.js",
        source: br#"
class Foo {
  constructor(x) { this.x = x; }
  bump() { if (this.x > 0) this.x++; }
}
function main() { const f = new Foo(1); f.bump(); }
"#,
    },
    Sample {
        lang: "typescript",
        path: "mod.ts",
        source: br#"
class Foo {
  constructor(public x: number) {}
  bump(): void { if (this.x > 0) this.x++; }
}
function main(): void { const f = new Foo(1); f.bump(); }
"#,
    },
    Sample {
        lang: "go",
        path: "mod.go",
        source: br#"
package main
type Foo struct { X int }
func NewFoo(x int) *Foo { return &Foo{X: x} }
func (f *Foo) Bump() { if f.X > 0 { f.X++ } }
func main() { f := NewFoo(1); f.Bump() }
"#,
    },
    Sample {
        lang: "java",
        path: "Foo.java",
        source: br#"
public class Foo {
  int x;
  public Foo(int x) { this.x = x; }
  public void bump() { if (x > 0) x++; }
  public static void main(String[] a) { new Foo(1).bump(); }
}
"#,
    },
    Sample {
        lang: "c",
        path: "mod.c",
        source: br#"
typedef struct { int x; } Foo;
Foo foo_new(int x) { Foo f; f.x = x; return f; }
void foo_bump(Foo *f) { if (f->x > 0) f->x++; }
int main(void) { Foo f = foo_new(1); foo_bump(&f); return 0; }
"#,
    },
    Sample {
        lang: "cpp",
        path: "mod.cpp",
        source: br#"
struct Foo { int x; Foo(int x): x(x) {} void bump() { if (x > 0) ++x; } };
int main() { Foo f(1); f.bump(); return 0; }
"#,
    },
    Sample {
        lang: "csharp",
        path: "Foo.cs",
        source: br#"
public class Foo {
  public int X;
  public Foo(int x) { X = x; }
  public void Bump() { if (X > 0) X++; }
  static void Main() { new Foo(1).Bump(); }
}
"#,
    },
    Sample {
        lang: "ruby",
        path: "foo.rb",
        source: br#"
class Foo
  def initialize(x); @x = x; end
  def bump; @x += 1 if @x > 0; end
end
Foo.new(1).bump
"#,
    },
    Sample {
        lang: "php",
        path: "Foo.php",
        source: br#"
<?php
class Foo {
  public function __construct(private int $x) {}
  public function bump(): void { if ($this->x > 0) $this->x++; }
}
(new Foo(1))->bump();
"#,
    },
    Sample {
        lang: "kotlin",
        path: "Foo.kt",
        source: br#"
class Foo(var x: Int) {
  fun bump() { if (x > 0) x++ }
}
fun main() { Foo(1).bump() }
"#,
    },
    Sample {
        lang: "groovy",
        path: "Foo.groovy",
        source: br#"
class Foo {
  int x
  Foo(int x) { this.x = x }
  void bump() { if (x > 0) x++ }
}
new Foo(1).bump()
"#,
    },
];

fn bench_langs(c: &mut Criterion) {
    let registry = default_registry();
    let mut group = c.benchmark_group("extract_all_vs_multipass_api");
    group.measurement_time(Duration::from_secs(3));
    group.warm_up_time(Duration::from_millis(500));

    for sample in SAMPLES {
        let Some(plugin) = registry.get_language_plugin(sample.lang) else {
            eprintln!("skip missing plugin {}", sample.lang);
            continue;
        };
        let path = Path::new(sample.path);
        let source = sample.source;

        group.bench_with_input(
            BenchmarkId::new("single_pass_extract_all", sample.lang),
            &(),
            |b, _| {
                b.iter(|| plugin.extract_all(path, source).expect("extract_all"));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("multipass_symbols_plus_relations", sample.lang),
            &(),
            |b, _| {
                b.iter(|| {
                    let symbols = plugin.extract_symbols(path, source).expect("symbols");
                    plugin
                        .extract_relations(path, source, &symbols)
                        .expect("relations")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_langs);
criterion_main!(benches);
