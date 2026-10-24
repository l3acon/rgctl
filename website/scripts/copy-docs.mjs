import {
  cpSync,
  existsSync,
  mkdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "../..");
const srcDocs = join(repoRoot, "docs");
const destDocs = join(here, "../content/docs");
const publicDir = join(here, "../public");

if (!existsSync(srcDocs)) {
  console.warn("[copy-docs] missing docs/ — skip");
  process.exit(0);
}

// Clean slate so deleted/renamed docs do not linger in static builds.
if (existsSync(destDocs)) {
  rmSync(destDocs, { recursive: true, force: true });
}
mkdirSync(destDocs, { recursive: true });
cpSync(srcDocs, destDocs, {
  recursive: true,
  filter: (src) => {
    const rel = relative(srcDocs, src);
    if (!rel) return true;
    if (rel.startsWith("internal")) return false;
    if (rel.startsWith("videos")) return false;
    if (rel === "languages" || rel.startsWith("languages/")) return false;
    // keep md/txt/images
    if (statSync(src).isDirectory()) return true;
    return (
      src.endsWith(".md") ||
      src.endsWith(".txt") ||
      /\.(png|jpg|jpeg|gif|svg|webp)$/i.test(src)
    );
  },
});

// Also copy AGENTS.md as agents.md for site
const agents = join(repoRoot, "AGENTS.md");
if (existsSync(agents)) {
  cpSync(agents, join(destDocs, "AGENTS.md"));
}

const installSh = join(repoRoot, "scripts/install.sh");
if (existsSync(installSh)) {
  mkdirSync(publicDir, { recursive: true });
  cpSync(installSh, join(publicDir, "install.sh"));
  console.log("[copy-docs] scripts/install.sh → website/public/install.sh");
}

const llmsTxt = `# rgctl

> Open-source code knowledge graph for LLM agents. Index once (\`discover\`), then query compact JSON — callers, blast radius, CFG/PDG slices, communities, semantic search, OSV reachability, and migration planning.

Website: https://shaaf.dev/rgctl/
Repository: https://github.com/sshaaf/rgctl
Install: \`curl -fsSL https://shaaf.dev/rgctl/install.sh | sh\`

## Capabilities

- Typed code knowledge graph: Tier-1 deep-analysis languages (C, C++, C#, Go, Groovy, Java, JavaScript, Kotlin, PHP, Puppet, Python, Ruby, Rust, TypeScript; plus ERB / Markdown) and the full \`languages.toml\` plugin catalog (config/IaC and additional languages)
- Structured queries: \`find\`, \`callers\`, \`callees\`, \`relations\`, \`inventory\`
- Blast radius; Fast-Leiden communities; PageRank / betweenness / harmonic metrics; git churn (\`discover --with-git\`, \`metrics --churn\`)
- Hybrid CPG / CFG / PDG: \`rgctl slice\` builds single-function CFG/PDG lazily on demand; \`discover --with-cfg\` pre-builds the full repo archive
- Cross-language MRO (\`relations -e overrides\`), SCIP overlay (\`discover --scip\`), frameworks / DI routes
- Semantic search over functions and docs
- OSV / vuln reachability and OpenVEX
- Agent skills via \`rgctl install --skill\`

## CLI (JSON-first)

- \`rgctl discover .\` — index repo → \`.rgctl/\` (optional \`--with-git\`, \`--scip <PATH.scip>\`, \`--with-cfg\`)
- \`rgctl update\` / \`rgctl watch\` / \`rgctl merge\` — incremental refresh, file watch, multi-repo park-and-resolve
- \`rgctl -f json find|callers|callees|relations|inventory|blast-radius|cpg|slice|metrics|status\`
- \`rgctl relations <SYM> -e overrides\` / \`rgctl relations <FILE> -e cochangeswith\`
- \`rgctl metrics --pagerank\` / \`--churn\` / \`--communities\`
- \`rgctl slice <FILE> --line <N> --variable <VAR>\` — lazy CFG/PDG (optional \`--with-alias\`, \`--taint\`)
- \`rgctl clones --mode exact|bloom|fragment\` — Type 1–4 (SESE + 1-WL fragment)
- \`rgctl semantic index\` / \`rgctl semantic query\`
- \`rgctl review paths\` / \`rgctl review check\` (PR gates; \`pr-check\` is a compatibility alias)
- \`rgctl rules\` / \`discover --with-kantra\` — Konveyor-style rules

## Guides

- https://shaaf.dev/rgctl/docs/guides/
- https://shaaf.dev/rgctl/docs/guides/cli-command-reference/
- https://shaaf.dev/rgctl/docs/guides/discovering-and-indexing/
- https://shaaf.dev/rgctl/docs/guides/agent-skill/
- https://shaaf.dev/rgctl/docs/guides/dependency-injection-and-frameworks/
- https://shaaf.dev/rgctl/docs/guides/blast-radius-analysis/
- https://shaaf.dev/rgctl/docs/guides/hybrid-cpg/
- https://shaaf.dev/rgctl/docs/guides/program-slicing/
- https://shaaf.dev/rgctl/docs/guides/clone-detection/
- https://shaaf.dev/rgctl/docs/guides/community-detection/
- https://shaaf.dev/rgctl/docs/guides/vulnerability-and-osv-analysis/
- https://shaaf.dev/rgctl/docs/guides/migration-planning/

## Reference

- https://shaaf.dev/rgctl/docs/user-guide/
- https://shaaf.dev/rgctl/docs/json-api/
- https://shaaf.dev/rgctl/docs/policy-format/
- https://shaaf.dev/rgctl/docs/languages/

## Optional

- https://shaaf.dev/rgctl/docs/contributor-checklist/
- https://shaaf.dev/rgctl/docs/analysis-architecture/
- https://shaaf.dev/rgctl/llms.txt
`;

mkdirSync(publicDir, { recursive: true });
writeFileSync(join(publicDir, "llms.txt"), llmsTxt, "utf8");
console.log("[copy-docs] wrote website/public/llms.txt");

console.log(`[copy-docs] docs/ → website/content/docs (excl. internal/videos binaries)`);
