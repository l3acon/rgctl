import type { Metadata } from "next";
import Link from "next/link";
import { Badge } from "@/components/ui/badge";
import {
  coverageHandledCount,
  formatExtensions,
  HANDLER_ORDER,
  listLanguages,
} from "@/lib/languages";
import { GITHUB_REPO } from "@/lib/utils";

export const metadata: Metadata = {
  title: "Languages · Docs",
};

export default function LanguagesIndexPage() {
  const languages = listLanguages();

  return (
    <div className="mx-auto max-w-6xl px-4 py-14 sm:px-6">
      <Badge className="mb-4">Language support</Badge>
      <h1 className="text-3xl tracking-tight text-[var(--ink)] sm:text-4xl">
        Languages
      </h1>
      <p className="mt-3 max-w-2xl text-[var(--body)]">
        Generated at build time from each plugin&apos;s{" "}
        <code className="text-sm">*-ast-coverage.json</code> (single source of
        truth) plus extensions from{" "}
        <code className="text-sm">languages.toml</code>. Handlers describe how
        named tree-sitter kinds map into the graph.
      </p>
      <p className="mt-2 text-sm text-[var(--mute)]">
        Contributor bar:{" "}
        <Link href="/docs/tier-1-language-support/" className="underline">
          Tier 1 language support
        </Link>
        . Manifests live under{" "}
        <a
          href={`${GITHUB_REPO}/tree/main/crates`}
          className="underline"
          target="_blank"
          rel="noreferrer"
        >
          crates/rgctl-lang-*
        </a>
        .
      </p>

      <div className="mt-8 overflow-x-auto">
        <table className="w-full min-w-[40rem] border-collapse text-sm">
          <thead>
            <tr className="border-b border-[var(--hairline)] text-left text-[var(--mute)]">
              <th className="p-2 font-medium">Language</th>
              <th className="p-2 font-medium">Extensions</th>
              <th className="p-2 font-medium">Grammar</th>
              <th className="p-2 font-medium">Kinds</th>
              <th className="p-2 font-medium">Handled</th>
              <th className="p-2 font-medium">Skip</th>
            </tr>
          </thead>
          <tbody>
            {languages.map((lang) => {
              const handled = coverageHandledCount(lang.handlerCounts);
              const skip = lang.handlerCounts.Skip ?? 0;
              return (
                <tr
                  key={lang.id}
                  className="border-b border-[var(--hairline)] text-[var(--body)]"
                >
                  <td className="p-2">
                    <Link
                      href={`/docs/languages/${lang.id}/`}
                      className="font-medium text-[var(--ink)] underline"
                    >
                      {lang.displayName}
                    </Link>
                  </td>
                  <td className="p-2 font-mono text-xs">
                    {formatExtensions(lang.extensions)}
                  </td>
                  <td className="p-2 font-mono text-xs">{lang.grammar || "—"}</td>
                  <td className="p-2">{lang.kindCount}</td>
                  <td className="p-2">{handled}</td>
                  <td className="p-2">{skip}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <h2 className="mt-12 text-lg font-medium text-[var(--ink)]">
        Handler legend
      </h2>
      <ul className="mt-3 grid gap-2 text-sm text-[var(--body)] sm:grid-cols-2">
        {HANDLER_ORDER.map((h) => (
          <li key={h} className="rounded-[4px] border border-[var(--hairline)] p-3">
            <span className="font-mono text-[var(--ink)]">{h}</span>
            <span className="mt-1 block text-[var(--mute)]">
              {handlerBlurb(h)}
            </span>
          </li>
        ))}
      </ul>

      {!languages.length && (
        <p className="mt-8 text-sm text-[var(--mute)]">
          No coverage catalog found. Run{" "}
          <code className="text-xs">node scripts/copy-lang-coverage.mjs</code>{" "}
          from <code className="text-xs">website/</code> (also runs on{" "}
          <code className="text-xs">predev</code> / <code className="text-xs">prebuild</code>
          ).
        </p>
      )}
    </div>
  );
}

function handlerBlurb(h: string): string {
  switch (h) {
    case "Symbol":
      return "Emits graph symbols / nodes (functions, types, …).";
    case "Relation":
      return "Emits typed edges (calls, imports, heritage, …).";
    case "CfgStatement":
      return "Feeds CFG / control-flow construction.";
    case "AstSkeleton":
      return "Kept for analysis skeleton / field-write paths.";
    case "Literal":
      return "Leaf / literal tokens walked but not promoted to symbols.";
    case "Skip":
      return "Named grammar kind intentionally not mapped.";
    default:
      return "";
  }
}
