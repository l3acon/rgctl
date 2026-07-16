"""Intentional call graph for rBuilder expected-facts checks."""


def correctness_leaf() -> int:
    """Leaf — no outbound application calls."""
    return 42


def correctness_mid() -> int:
    """Mid — calls correctness_leaf."""
    return correctness_leaf() + 1


def correctness_root(flag: bool) -> int:
    """Root — calls correctness_mid and branches for a non-trivial CFG."""
    value = correctness_mid()
    if flag:
        return value * 2
    return value
