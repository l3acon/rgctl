import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const contentRoot = join(process.cwd(), "content/languages");

export const HANDLER_ORDER = [
  "Symbol",
  "Relation",
  "CfgStatement",
  "AstSkeleton",
  "Literal",
  "Skip",
] as const;

export type HandlerKind = (typeof HANDLER_ORDER)[number] | string;

export type LanguageCatalogEntry = {
  id: string;
  displayName: string;
  grammar: string;
  crateDir: string;
  manifestFile: string;
  plugin: string;
  grammarCrate: string;
  handler: string;
  extensions: string[];
  aliases: string[];
  kindCount: number;
  handlerCounts: Record<string, number>;
};

export type AstCoverageManifest = {
  grammar: string;
  handlers: Record<string, string>;
};

export type LanguageCatalog = {
  generated_from: string;
  languages: LanguageCatalogEntry[];
};

function readJson<T>(path: string): T | null {
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

export function languagesContentRoot(): string {
  return contentRoot;
}

export function listLanguages(): LanguageCatalogEntry[] {
  const catalog = readJson<LanguageCatalog>(join(contentRoot, "catalog.json"));
  if (!catalog?.languages?.length) return [];
  return [...catalog.languages].sort((a, b) =>
    a.displayName.localeCompare(b.displayName),
  );
}

export function getLanguage(id: string): LanguageCatalogEntry | null {
  return listLanguages().find((l) => l.id === id) ?? null;
}

export function loadCoverage(id: string): AstCoverageManifest | null {
  return readJson<AstCoverageManifest>(
    join(contentRoot, `${id}-ast-coverage.json`),
  );
}

export function groupHandlers(
  handlers: Record<string, string>,
): Map<string, string[]> {
  const groups = new Map<string, string[]>();
  for (const kind of HANDLER_ORDER) {
    groups.set(kind, []);
  }
  for (const [nodeKind, handler] of Object.entries(handlers)) {
    const list = groups.get(handler) ?? [];
    list.push(nodeKind);
    groups.set(handler, list);
  }
  for (const list of groups.values()) {
    list.sort((a, b) => a.localeCompare(b));
  }
  return groups;
}

export function formatExtensions(exts: string[]): string {
  if (!exts.length) return "—";
  return exts.map((e) => (e.startsWith(".") ? e : `.${e}`)).join(", ");
}

export function coverageHandledCount(counts: Record<string, number>): number {
  let n = 0;
  for (const [k, v] of Object.entries(counts)) {
    if (k !== "Skip") n += v;
  }
  return n;
}
