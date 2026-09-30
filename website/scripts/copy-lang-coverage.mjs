/**
 * Copy `*-ast-coverage.json` (+ languages.toml metadata) into
 * `website/content/languages/` so the site can render language support
 * from the repo SSOT at build time.
 */
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "../..");
const cratesDir = join(repoRoot, "crates");
const destDir = join(here, "../content/languages");
const languagesToml = join(repoRoot, "languages.toml");

/** Display names for language ids. */
const DISPLAY = {
  c: "C",
  cpp: "C++",
  csharp: "C#",
  go: "Go",
  groovy: "Groovy",
  java: "Java",
  javascript: "JavaScript",
  kotlin: "Kotlin",
  markdown: "Markdown",
  php: "PHP",
  puppet: "Puppet",
  python: "Python",
  ruby: "Ruby",
  rust: "Rust",
  typescript: "TypeScript",
};

/**
 * Minimal parse of `[languages.<id>]` tables we care about.
 * @returns {Record<string, { extensions: string[], aliases: string[], plugin: string, crate: string, handler: string }>}
 */
function parseLanguagesToml(text) {
  /** @type {Record<string, any>} */
  const out = {};
  let current = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const table = line.match(/^\[languages\.([a-z0-9_]+)\]$/i);
    if (table) {
      current = table[1];
      out[current] = {
        extensions: [],
        aliases: [],
        plugin: "",
        crate: "",
        handler: "",
      };
      continue;
    }
    if (!current || line.startsWith("[")) {
      current = null;
      continue;
    }
    const kv = line.match(/^([a-z_]+)\s*=\s*(.+)$/i);
    if (!kv) continue;
    const key = kv[1];
    let val = kv[2].trim();
    if (val.startsWith("[")) {
      const items = [...val.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
      if (key === "extensions" || key === "aliases") {
        out[current][key] = items;
      }
    } else if (val.startsWith('"')) {
      val = val.replace(/^"|"$/g, "");
      if (key === "plugin" || key === "crate" || key === "handler") {
        out[current][key] = val;
      }
    }
  }
  return out;
}

function summarizeHandlers(handlers) {
  /** @type {Record<string, number>} */
  const counts = {};
  for (const h of Object.values(handlers)) {
    counts[h] = (counts[h] || 0) + 1;
  }
  return counts;
}

if (existsSync(destDir)) {
  rmSync(destDir, { recursive: true, force: true });
}
mkdirSync(destDir, { recursive: true });

const meta = existsSync(languagesToml)
  ? parseLanguagesToml(readFileSync(languagesToml, "utf8"))
  : {};

/** @type {Array<Record<string, unknown>>} */
const catalog = [];

for (const name of readdirSync(cratesDir).sort()) {
  if (!name.startsWith("rgctl-lang-")) continue;
  const id = name.slice("rgctl-lang-".length);
  const manifest = join(cratesDir, name, `${id}-ast-coverage.json`);
  if (!existsSync(manifest)) continue;

  const dest = join(destDir, `${id}-ast-coverage.json`);
  cpSync(manifest, dest);

  const coverage = JSON.parse(readFileSync(manifest, "utf8"));
  const handlers = coverage.handlers || {};
  const counts = summarizeHandlers(handlers);
  const langMeta = meta[id] || {};

  catalog.push({
    id,
    displayName: DISPLAY[id] || id,
    grammar: coverage.grammar || "",
    crateDir: name,
    manifestFile: `${id}-ast-coverage.json`,
    plugin: langMeta.plugin || "",
    grammarCrate: langMeta.crate || "",
    handler: langMeta.handler || "custom",
    extensions: langMeta.extensions || [],
    aliases: langMeta.aliases || [],
    kindCount: Object.keys(handlers).length,
    handlerCounts: counts,
  });
}

writeFileSync(
  join(destDir, "catalog.json"),
  JSON.stringify({ generated_from: "*-ast-coverage.json", languages: catalog }, null, 2) +
    "\n",
);

writeFileSync(
  join(destDir, "languages-meta.json"),
  JSON.stringify({ source: "languages.toml", languages: meta }, null, 2) + "\n",
);

console.log(
  `[copy-lang-coverage] ${catalog.length} language(s) → website/content/languages/`,
);
