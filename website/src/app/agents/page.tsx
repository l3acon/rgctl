import type { Metadata } from "next";
import Link from "next/link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { TerminalBlock } from "@/components/terminal";
import { GITHUB_REPO } from "@/lib/utils";

export const metadata: Metadata = {
  title: "Agents",
};

const steps = [
  {
    n: "1",
    title: "Install the skill",
    body: "rgctl install --skill writes a single skill named rgctl into your IDE skill dirs.",
  },
  {
    n: "2",
    title: "Discover once",
    body: "rgctl discover . builds {repo}/.rgctl/ — later questions are graph lookups, not full-repo reads.",
  },
  {
    n: "3",
    title: "Ask in natural language",
    body: "The agent loads the rgctl skill and runs rgctl -f json structured verbs.",
  },
  {
    n: "4",
    title: "Reason from JSON",
    body: "Parse schema_version on stdout — then edit, and re-check with check / pr-check when needed.",
  },
];

const topics = [
  {
    title: "Index",
    blurb: "Build or refresh the graph.",
    cli: "discover",
  },
  {
    title: "Impact",
    blurb: "Upstream blast radius and callers.",
    cli: "blast-radius · callers",
  },
  {
    title: "Data flow",
    blurb: "Slices, mutations, CPG flows.",
    cli: "slice · cpg",
  },
  {
    title: "Search",
    blurb: "Symbol lookup and semantic query.",
    cli: "find · semantic query",
  },
  {
    title: "Migration",
    blurb: "Dependency-aware extraction order.",
    cli: "discover --export-migration-hints",
  },
  {
    title: "Kantra",
    blurb: "Konveyor rules and violations.",
    cli: "discover --with-kantra",
  },
  {
    title: "CI gates",
    blurb: "Policy checks on PRs and diffs.",
    cli: "check · pr-check",
  },
];

export default function AgentsPage() {
  return (
    <div className="mx-auto max-w-6xl px-4 py-14 sm:px-6">
      <Badge className="mb-4">Agent pack</Badge>
      <h1 className="text-3xl tracking-tight text-[var(--ink)] sm:text-4xl">
        Built for coding agents
      </h1>
      <p className="mt-3 max-w-2xl text-[var(--body)]">
        Install <strong className="font-medium text-[var(--ink)]">one skill</strong>{" "}
        — <code className="font-mono text-sm">rgctl</code> — not a family of
        slash commands or per-workflow skill folders. It teaches the agent to
        answer structural questions with{" "}
        <code className="font-mono text-sm">rgctl -f json</code> instead of
        dumping whole files into context.
      </p>

      <section className="mt-10 space-y-3">
        <h2 className="text-lg text-[var(--ink)]">Install into your repo</h2>
        <TerminalBlock
          lines={[
            "cd /path/to/your-app",
            "rgctl install --skill --tools cursor,claude,codex,antigravity,agents",
            "rgctl discover .",
            "# Optional: remove leftover rgctl-discover / rgctl-impact / … dirs from older packs",
          ]}
        />
        <p className="text-sm text-[var(--mute)]">
          Writes{" "}
          <code className="font-mono">.cursor/skills/rgctl/</code> (and the
          equivalent path for other adapters). Optional{" "}
          <code className="font-mono">--with-policy</code> adds a Cursor
          structural-rules snippet. Full flags:{" "}
          <Link href="/docs/guides/agent-skill/" className="underline">
            agent pack guide
          </Link>
          .
        </p>
      </section>

      <ol className="mt-10 grid gap-4 sm:grid-cols-2">
        {steps.map((s) => (
          <li
            key={s.n}
            className="rounded-[4px] border border-[var(--hairline)] p-4"
          >
            <p className="font-mono text-[11px] text-[var(--mute)]">
              Step {s.n}
            </p>
            <h2 className="mt-1 text-base text-[var(--ink)]">{s.title}</h2>
            <p className="mt-1 text-sm text-[var(--body)]">{s.body}</p>
          </li>
        ))}
      </ol>

      <section className="mt-12">
        <h2 className="text-lg text-[var(--ink)]">What the skill covers</h2>
        <p className="mt-2 max-w-2xl text-sm text-[var(--body)]">
          Routing tables and worked scenarios live inside the skill (
          <code className="font-mono text-xs">references/workflows.md</code>
          ). The engine is still the terminal CLI.
        </p>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {topics.map((w) => (
            <div
              key={w.title}
              className="flex flex-col rounded-[4px] border border-[var(--hairline)] bg-[var(--canvas-soft)]/50 p-5"
            >
              <h3 className="text-base font-medium text-[var(--ink)]">
                {w.title}
              </h3>
              <p className="mt-2 text-sm text-[var(--body)]">{w.blurb}</p>
              <p className="mt-3 font-mono text-[11px] text-[var(--mute)]">
                {w.cli}
              </p>
            </div>
          ))}
        </div>
      </section>

      <section className="mt-12 space-y-3">
        <h2 className="text-lg text-[var(--ink)]">Minimal agent loop</h2>
        <TerminalBlock
          lines={[
            'export REPO=/path/to/repo',
            'cd "$REPO" && rgctl discover .',
            'rgctl -r "$REPO" -f json find --type function --count-only | jq \'.total\'',
            'rgctl -r "$REPO" -f json find priceShoppingCart --exact | jq \'.entities[0]\'',
            'rgctl -r "$REPO" -f json blast-radius priceShoppingCart \\',
            "  | jq '{score: .metrics.score, callers: .metrics.direct_callers_count}'",
          ]}
        />
      </section>

      <section className="mt-10 flex flex-wrap gap-3">
        <Button asChild>
          <Link href="/docs/guides/agent-skill/">Agent pack guide</Link>
        </Button>
        <Button variant="ghost" asChild>
          <Link href="/docs/guides/structured-query/">Structured queries</Link>
        </Button>
        <Button variant="ghost" asChild>
          <a
            href={`${GITHUB_REPO}/blob/main/docs/agents/USER_AGENTS_TEMPLATE.md`}
            target="_blank"
            rel="noreferrer"
          >
            USER_AGENTS_TEMPLATE
          </a>
        </Button>
        <Button variant="ghost" asChild>
          <Link href="/install/">Install rgctl</Link>
        </Button>
      </section>
    </div>
  );
}
