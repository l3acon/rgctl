//! Folded `extract_all` complexity must match standalone `calculate_complexity`.

use rgctl_plugin_api::{LanguagePlugin, SymbolType, complexity_from_metadata};
use std::path::Path;
use std::sync::Arc;

struct LangFixture {
    language_id: &'static str,
    rel_path: &'static str,
    source: &'static [u8],
}

/// Multi-branch fixtures for languages known to fold complexity in single-pass extract.
const FIXTURES: &[LangFixture] = &[
    LangFixture {
        language_id: "rust",
        rel_path: "parity.rs",
        source: br#"
fn branched(x: i32) -> i32 {
    if x > 0 {
        if x > 10 {
            return x;
        }
        return 1;
    } else if x < 0 {
        for _ in 0..3 {
            if x == -1 {
                break;
            }
        }
        return -1;
    }
    match x {
        0 => 0,
        _ => x,
    }
}
"#,
    },
    LangFixture {
        language_id: "python",
        rel_path: "parity.py",
        source: br#"
def branched(x):
    if x > 0:
        if x > 10:
            return x
        return 1
    elif x < 0:
        for _ in range(3):
            if x == -1:
                break
        return -1
    while False:
        pass
    return 0
"#,
    },
    LangFixture {
        language_id: "java",
        rel_path: "Parity.java",
        source: br#"
public class Parity {
    public int branched(int x) {
        if (x > 0) {
            if (x > 10) {
                return x;
            }
            return 1;
        } else if (x < 0) {
            for (int i = 0; i < 3; i++) {
                if (x == -1) {
                    break;
                }
            }
            return -1;
        }
        switch (x) {
            case 0:
                return 0;
            default:
                return x;
        }
    }
}
"#,
    },
    LangFixture {
        language_id: "typescript",
        rel_path: "parity.ts",
        source: br#"
export function branched(x: number): number {
    if (x > 0) {
        if (x > 10) {
            return x;
        }
        return 1;
    } else if (x < 0) {
        for (let i = 0; i < 3; i++) {
            if (x === -1) {
                break;
            }
        }
        return -1;
    }
    switch (x) {
        case 0:
            return 0;
        default:
            return x;
    }
}
"#,
    },
    LangFixture {
        language_id: "go",
        rel_path: "parity.go",
        source: br#"
package parity

func Branched(x int) int {
	if x > 0 {
		if x > 10 {
			return x
		}
		return 1
	} else if x < 0 {
		for i := 0; i < 3; i++ {
			if x == -1 {
				break
			}
		}
		return -1
	}
	switch x {
	case 0:
		return 0
	default:
		return x
	}
}
"#,
    },
    // javascript folds complexity similarly to typescript; included when present in registry.
    LangFixture {
        language_id: "javascript",
        rel_path: "parity.js",
        source: br#"
export function branched(x) {
    if (x > 0) {
        if (x > 10) {
            return x;
        }
        return 1;
    } else if (x < 0) {
        for (let i = 0; i < 3; i++) {
            if (x === -1) {
                break;
            }
        }
        return -1;
    }
    switch (x) {
        case 0:
            return 0;
        default:
            return x;
    }
}
"#,
    },
    LangFixture {
        language_id: "c",
        rel_path: "parity.c",
        source: br#"
int branched(int x) {
    if (x > 0) {
        if (x > 10) {
            return x;
        }
        return 1;
    } else if (x < 0) {
        for (int i = 0; i < 3; i++) {
            if (x == -1) {
                break;
            }
        }
        return -1;
    }
    return 0;
}
"#,
    },
    LangFixture {
        language_id: "cpp",
        rel_path: "parity.cpp",
        source: br#"
int branched(int x) {
    if (x > 0) {
        if (x > 10) {
            return x;
        }
        return 1;
    } else if (x < 0) {
        for (int i = 0; i < 3; i++) {
            if (x == -1) {
                break;
            }
        }
        return -1;
    }
    return 0;
}
"#,
    },
    LangFixture {
        language_id: "csharp",
        rel_path: "Parity.cs",
        source: br#"
public class Parity {
    public int Branched(int x) {
        if (x > 0) {
            if (x > 10) {
                return x;
            }
            return 1;
        } else if (x < 0) {
            return -1;
        }
        return 0;
    }
}
"#,
    },
    LangFixture {
        language_id: "ruby",
        rel_path: "parity.rb",
        source: br#"
def branched(x)
  if x > 0
    if x > 10
      return x
    end
    return 1
  elsif x < 0
    return -1
  end
  0
end
"#,
    },
    LangFixture {
        language_id: "php",
        rel_path: "parity.php",
        source: br#"<?php
function branched($x) {
    if ($x > 0) {
        if ($x > 10) {
            return $x;
        }
        return 1;
    } elseif ($x < 0) {
        return -1;
    }
    return 0;
}
"#,
    },
    LangFixture {
        language_id: "kotlin",
        rel_path: "Parity.kt",
        source: br#"
fun branched(x: Int): Int {
    if (x > 0) {
        if (x > 10) {
            return x
        }
        return 1
    } else if (x < 0) {
        return -1
    }
    return 0
}
"#,
    },
    LangFixture {
        language_id: "swift",
        rel_path: "Parity.swift",
        source: br#"
func branched(_ x: Int) -> Int {
    if x > 0 {
        if x > 10 {
            return x
        }
        return 1
    } else if x < 0 {
        return -1
    }
    return 0
}
"#,
    },
    LangFixture {
        language_id: "scala",
        rel_path: "Parity.scala",
        source: br#"
object Parity {
  def branched(x: Int): Int = {
    if (x > 0) {
      if (x > 10) return x
      return 1
    } else if (x < 0) {
      return -1
    }
    0
  }
}
"#,
    },
    LangFixture {
        language_id: "dart",
        rel_path: "parity.dart",
        source: br#"
int branched(int x) {
  if (x > 0) {
    if (x > 10) {
      return x;
    }
    return 1;
  } else if (x < 0) {
    return -1;
  }
  return 0;
}
"#,
    },
    LangFixture {
        language_id: "elixir",
        rel_path: "parity.ex",
        source: br#"
defmodule Parity do
  def branched(x) do
    if x > 0 do
      x
    else
      0
    end
  end
end
"#,
    },
    LangFixture {
        language_id: "lua",
        rel_path: "parity.lua",
        source: br#"
function branched(x)
  if x > 0 then
    if x > 10 then
      return x
    end
    return 1
  elseif x < 0 then
    return -1
  end
  return 0
end
"#,
    },
    LangFixture {
        language_id: "bash",
        rel_path: "parity.sh",
        source: br#"#!/usr/bin/env bash
branched() {
  if [ "$1" -gt 0 ]; then
    echo 1
  else
    echo 0
  fi
}
"#,
    },
    LangFixture {
        language_id: "groovy",
        rel_path: "Parity.groovy",
        source: br#"
class Parity {
    int branched(int x) {
        if (x > 0) {
            return x
        }
        return 0
    }
}
"#,
    },
];

fn assert_parity(plugin: Arc<dyn LanguagePlugin>, fixture: &LangFixture) {
    let path = Path::new(fixture.rel_path);
    let result = plugin
        .extract_all(path, fixture.source)
        .unwrap_or_else(|e| panic!("{} extract_all: {e}", fixture.language_id));

    let mut checked = 0usize;
    for symbol in &result.symbols {
        if !matches!(
            symbol.symbol_type,
            SymbolType::Function | SymbolType::Method
        ) {
            continue;
        }
        let Some(from_meta) = complexity_from_metadata(symbol) else {
            // Language registered but not folding complexity onto this symbol yet.
            continue;
        };
        let via_api = plugin
            .calculate_complexity(symbol, fixture.source)
            .unwrap_or_else(|e| {
                panic!(
                    "{} calculate_complexity({}): {e}",
                    fixture.language_id, symbol.name
                )
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} calculate_complexity returned None for {}",
                    fixture.language_id, symbol.name
                )
            });
        assert_eq!(
            via_api, from_meta,
            "{} symbol {} folded != calculate_complexity\n  folded={from_meta:?}\n  api={via_api:?}",
            fixture.language_id, symbol.name
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "{}: expected at least one Function/Method with rgctl_complexity metadata",
        fixture.language_id
    );
}

#[test]
fn single_pass_complexity_matches_calculate_complexity() {
    let registry = rgctl_languages::default_registry();
    let mut ran = 0usize;

    for fixture in FIXTURES {
        let Some(plugin) = registry.get_language_plugin(fixture.language_id) else {
            eprintln!("skip {}: plugin not in registry", fixture.language_id);
            continue;
        };
        if !plugin.capabilities().calculates_complexity {
            // Plugin does not advertise complexity (e.g. stub / query-only).
            eprintln!("skip {}: calculates_complexity=false", fixture.language_id);
            continue;
        }
        assert_parity(plugin, fixture);
        ran += 1;
    }

    assert!(
        ran >= 15,
        "expected ≥15 Tier-1 languages with complexity parity; ran={ran}"
    );
}
