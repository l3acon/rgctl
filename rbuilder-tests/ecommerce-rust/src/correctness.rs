//! Intentional call graph for rBuilder expected-facts checks.
//! Prefer direct function calls so extraction does not depend on DI.

#![allow(dead_code)]

/// Leaf — no outbound application calls.
pub fn correctness_leaf() -> i32 {
    42
}

/// Mid — calls [`correctness_leaf`].
pub fn correctness_mid() -> i32 {
    correctness_leaf() + 1
}

/// Root — calls [`correctness_mid`] and branches for a non-trivial CFG.
pub fn correctness_root(flag: bool) -> i32 {
    let value = correctness_mid();
    if flag {
        value * 2
    } else {
        value
    }
}
