#!/usr/bin/env bash
# Fetch all large local example repos used by profiling/testing (gitignored under /example).
# Includes:
# - linux (kernel)
# - kafka
# - metasfresh
# - coolstore-weblogic
# - kubernetes
# - magento2 (Magento Open Source — PHP migration stress corpus)
# - k8s-website (kubernetes/website content/en via sparse checkout)
# - rust (rust-lang/rust — Rust language-scale cold profile)
# - home-assistant (Python ~12k files)
# - discourse (Ruby app/lib/plugins — language-scale cold profile)
# - vscode (TypeScript in src/)
# - node (nodejs/node test/ — JavaScript language-scale corpus)
# - roslyn (C# compiler)
# - llvm-project (C++ via sparse clang/)
# - kotlin (JetBrains/kotlin sparse libraries+plugins+analysis — Kotlin Gate B)
# - groovy (gradle/gradle — Groovy Gate B; largest single OSS .groovy tree)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXAMPLE_DIR="$ROOT/example"
TMP_DIR="$EXAMPLE_DIR/.tmp-fetch"
mkdir -p "$EXAMPLE_DIR" "$TMP_DIR"

clone_if_missing() {
  local url="$1"
  local dest="$2"
  local depth="${3:-1}"
  if [[ -d "$dest/.git" ]]; then
    echo "Already present: $dest"
    return 0
  fi
  if [[ -e "$dest" ]]; then
    echo "Skipping clone (path exists, not a git repo): $dest"
    return 0
  fi
  echo "Cloning: $url -> $dest"
  git clone --depth "$depth" "$url" "$dest"
}

clone_sparse_k8s_website_if_missing() {
  local dest="$1"
  local tmp="$TMP_DIR/k8s-website-clone"
  local url="https://github.com/kubernetes/website.git"
  if [[ -d "$dest/docs" || -f "$dest/search.md" ]]; then
    echo "Already present: $dest"
    return 0
  fi
  rm -rf "$tmp"
  echo "Cloning sparse kubernetes/website content/en -> $dest"
  git clone --depth 1 --filter=blob:none --sparse "$url" "$tmp"
  (
    cd "$tmp"
    git sparse-checkout set content/en
  )
  rm -rf "$dest"
  mv "$tmp/content/en" "$dest"
  rm -rf "$tmp"
}

# Full repos
clone_if_missing "https://github.com/torvalds/linux.git" "$EXAMPLE_DIR/linux"
clone_if_missing "https://github.com/apache/kafka.git" "$EXAMPLE_DIR/kafka"
clone_if_missing "https://github.com/metasfresh/metasfresh.git" "$EXAMPLE_DIR/metasfresh-4.9.8b"
clone_if_missing "https://github.com/konveyor-ecosystem/coolstore.git" "$EXAMPLE_DIR/coolstore-weblogic"
clone_if_missing "https://github.com/kubernetes/kubernetes.git" "$EXAMPLE_DIR/kubernetes"
clone_if_missing "https://github.com/magento/magento2.git" "$EXAMPLE_DIR/magento2"

# Sparse docs corpus
clone_sparse_k8s_website_if_missing "$EXAMPLE_DIR/k8s-website"

clone_sparse_llvm_clang_if_missing() {
  local dest="$1"
  local tmp="$TMP_DIR/llvm-project-clone"
  local url="https://github.com/llvm/llvm-project.git"
  if [[ -d "$dest/clang" ]]; then
    echo "Already present: $dest/clang"
    return 0
  fi
  rm -rf "$tmp"
  echo "Cloning sparse llvm/llvm-project clang/ -> $dest"
  git clone --depth 1 --filter=blob:none --sparse "$url" "$tmp"
  (
    cd "$tmp"
    git sparse-checkout set clang
  )
  rm -rf "$dest"
  mv "$tmp" "$dest"
  rm -rf "$TMP_DIR/llvm-project-clone"
}

# Language-scale corpora (~10k source files) — see openspec/changes/_shared/starting-context.md
clone_if_missing "https://github.com/rust-lang/rust.git" "$EXAMPLE_DIR/rust" 1
clone_if_missing "https://github.com/home-assistant/core.git" "$EXAMPLE_DIR/home-assistant" 1
clone_if_missing "https://github.com/discourse/discourse.git" "$EXAMPLE_DIR/discourse" 1
clone_if_missing "https://github.com/microsoft/vscode.git" "$EXAMPLE_DIR/vscode" 1
clone_if_missing "https://github.com/dotnet/roslyn.git" "$EXAMPLE_DIR/roslyn" 1
clone_sparse_llvm_clang_if_missing "$EXAMPLE_DIR/llvm-project"

# Puppet Gate B (~10⁴ .pp): deferred — no default monorepo yet.
# Override when baselining: RGCTL_PUPPET_REPO=/path/to/puppet/modules
# Suggested candidates: OpenStack puppet-* modules or a Forge module bundle under example/puppet.

clone_sparse_node_test_if_missing() {
  local dest="$1"
  local tmp="$TMP_DIR/node-clone"
  local url="https://github.com/nodejs/node.git"
  if [[ -d "$dest/test" ]]; then
    echo "Already present: $dest/test"
    return 0
  fi
  rm -rf "$tmp"
  echo "Cloning sparse nodejs/node test/ -> $dest"
  git clone --depth 1 --filter=blob:none --sparse "$url" "$tmp"
  (
    cd "$tmp"
    git sparse-checkout set test
  )
  rm -rf "$dest"
  mv "$tmp" "$dest"
  rm -rf "$TMP_DIR/node-clone"
}

clone_sparse_node_test_if_missing "$EXAMPLE_DIR/node"

# Kotlin Gate B: JetBrains/kotlin is huge; sparse libraries+plugins+analysis ≈ O(10⁴) .kt
# (full tree is 70k+ .kt / multi-GB). Override root with RGCTL_KOTLIN_REPO.
clone_sparse_kotlin_if_missing() {
  local dest="$1"
  local tmp="$TMP_DIR/kotlin-clone"
  local url="https://github.com/JetBrains/kotlin.git"
  if [[ -d "$dest/libraries" && -d "$dest/plugins" ]]; then
    echo "Already present: $dest (libraries+plugins)"
    return 0
  fi
  rm -rf "$tmp"
  echo "Cloning sparse JetBrains/kotlin libraries plugins analysis -> $dest"
  git clone --depth 1 --filter=blob:none --sparse "$url" "$tmp"
  (
    cd "$tmp"
    git sparse-checkout set libraries plugins analysis
  )
  rm -rf "$dest"
  mv "$tmp" "$dest"
  rm -rf "$TMP_DIR/kotlin-clone"
}

clone_sparse_kotlin_if_missing "$EXAMPLE_DIR/kotlin"

# Groovy Gate B: gradle/gradle is the densest single public .groovy tree (~6k; Jenkins core is tiny).
# Override with RGCTL_GROOVY_REPO. apache/groovy alone is ~3k.
clone_if_missing "https://github.com/gradle/gradle.git" "$EXAMPLE_DIR/groovy" 1

echo
echo "All requested example repos are available under: $EXAMPLE_DIR"
echo "Build: cargo build --release --bin rgctl"
echo "Cold profile gates:"
echo "  cargo test --release --test cold_profile_gates -- --ignored --nocapture --test-threads=1"
