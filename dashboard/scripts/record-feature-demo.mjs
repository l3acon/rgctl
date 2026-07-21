/**
 * Record an rBuilder dashboard feature montage — aligned with the User Guide.
 *
 * No on-page caption overlay (same idea as the CLI VHS tape). Captions are
 * burned later from docs/videos/rbuilder-feature-demo.srt via
 * docs/videos/burn-feature-demo-captions.sh.
 *
 * Prereq (ecommerce-java fixture recommended):
 *   rbuilder -r rbuilder-tests/ecommerce-java discover . -l java -e target \
 *     --with-cfg --with-security --with-taint --with-dashboard --with-harmonic \
 *     --export-migration-hints
 *   rbuilder -r rbuilder-tests/ecommerce-java semantic index --embedder vocab
 *   rbuilder -r rbuilder-tests/ecommerce-java serve --port 8080
 *
 * Usage:
 *   DASHBOARD_URL=http://127.0.0.1:8080/ node dashboard/scripts/record-feature-demo.mjs
 *
 * Outputs:
 *   docs/videos/rbuilder-feature-demo-no-captions.mp4
 *   docs/videos/rbuilder-feature-demo.srt
 *   docs/videos/rbuilder-feature-demo.raw.webm  (intermediate)
 */

import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const BASE = process.env.DASHBOARD_URL ?? "http://127.0.0.1:8080/";
const ROOT = path.resolve(import.meta.dirname, "../..");
const OUT_DIR = path.join(ROOT, "docs/videos");
const RAW_WEBM = path.join(OUT_DIR, "rbuilder-feature-demo.raw.webm");
const OUT_NO_CAPTIONS = path.join(OUT_DIR, "rbuilder-feature-demo-no-captions.mp4");
const OUT_SRT = path.join(OUT_DIR, "rbuilder-feature-demo.srt");
const SEC_PER_FEATURE = Number(process.env.DEMO_SEC_PER_FEATURE ?? "5");
const HOLD_MS = Math.round(SEC_PER_FEATURE * 1000);

/** Defaults match rbuilder-tests/ecommerce-java + user-guide walkthrough. */
const FN = process.env.CAPTURE_FN_DATAFLOW ?? "checkout";
const FN_BLAST = process.env.CAPTURE_FN_BLAST ?? "clearCart";
const FN_TAINT = process.env.CAPTURE_FN_TAINT ?? "checkout";
const FN_CFG = process.env.CAPTURE_FN_CFG ?? "checkout";
const FN_SLICE = process.env.CAPTURE_FN_SLICE ?? "addItem";
const SEMANTIC_QUERY = process.env.CAPTURE_SEMANTIC_QUERY ?? "shopping cart checkout";
const SLICE_LINE = process.env.CAPTURE_SLICE_LINE ?? "53";
const SLICE_VAR = process.env.CAPTURE_SLICE_VAR ?? "item";

/**
 * One segment per feature (order matches README / user-guide).
 * `caption` / `body` feed the SRT (not drawn in the page).
 */
const FEATURE_SEGMENTS = [
  {
    key: "discover",
    tab: null,
    panel: ".rb-stats-row",
    caption: "discover",
    body: "Graph snapshot & index metrics",
  },
  {
    key: "gql",
    tab: "Graph Visualization",
    panel: ".graph-panel.h-100",
    caption: "gql",
    body: "Package call graph — exact structure for agents",
  },
  {
    key: "semantic-search",
    tab: "Search",
    panel: ".search-view",
    caption: "semantic search",
    body: "Vocab / code-daemon index · Hamming + late fusion",
  },
  {
    key: "graph-metrics",
    tab: "Functions",
    panel: ".functions-view, .functions-table",
    caption: "metrics",
    body: "PageRank · betweenness · blast hotspots",
  },
  {
    key: "cfg",
    tab: "CFG / PDG Analysis",
    panel: ".cfg-detail, .cfg-graph-panel",
    caption: "inspect cfg",
    body: "Control-flow blocks & dominators",
  },
  {
    key: "pdg",
    tab: "Dataflow",
    panel: ".dataflow-graph-panel",
    caption: "inspect pdg",
    body: "Data & control dependencies",
  },
  {
    key: "dominance",
    tab: "Dataflow",
    panel: ".dataflow-graph-panel",
    caption: "inspect dom",
    body: "Dominator tree & frontiers",
  },
  {
    key: "program-slicing",
    tab: "Program Slicing",
    panel: ".slice-view",
    caption: "slice",
    body: "Backward slice — criterion & highlighted lines",
  },
  {
    key: "blast-radius",
    tab: "Blast Radius",
    panel: ".blast-view",
    caption: "blast-radius",
    body: "Upstream impact score & caller table",
  },
  {
    key: "taint",
    tab: "Taint Analysis",
    panel: ".taint-view",
    caption: "taint",
    body: "Source → sink flows",
  },
  {
    key: "migration",
    tab: "Migration",
    panel: ".migration-view, .migration-tuning",
    caption: "migration",
    body: "Package roadmap · presets & dual ordering",
  },
  {
    key: "ci-policy",
    tab: "Query Guide",
    panel: ".guide-view",
    caption: "check",
    body: "CI policy · blast-radius gates",
  },
  {
    key: "export",
    tab: "Query Guide",
    panel: ".guide-view",
    caption: "export",
    body: "GraphML · Mermaid · JSON subgraphs",
  },
];

const TARGET_SECS = FEATURE_SEGMENTS.length * SEC_PER_FEATURE;

fs.mkdirSync(OUT_DIR, { recursive: true });

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function resolveFfmpeg() {
  const full = "/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg";
  if (fs.existsSync(full)) return full;
  return "ffmpeg";
}

function writeSrt(outPath, segments, secPer) {
  const lines = [];
  const ts = (sec) => {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const s = Math.floor(sec % 60);
    const ms = Math.round((sec - Math.floor(sec)) * 1000);
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")},${String(ms).padStart(3, "0")}`;
  };
  segments.forEach((seg, i) => {
    const start = i * secPer;
    const end = (i + 1) * secPer;
    lines.push(String(i + 1));
    lines.push(`${ts(start)} --> ${ts(end)}`);
    lines.push(seg.caption);
    lines.push(seg.body);
    lines.push("");
  });
  fs.writeFileSync(outPath, lines.join("\n"));
}

async function clickTab(page, label) {
  const tab = page.locator(".rb-main-tabs").getByRole("button", { name: label, exact: true });
  await tab.scrollIntoViewIfNeeded();
  await tab.click();
  await sleep(350);
}

async function selectFunction(page, name) {
  const search = page.locator('.function-list-sidebar input[type="search"]');
  if (await search.count()) {
    await search.fill("");
    await sleep(120);
    await search.fill(name);
    await sleep(400);
  }
  const item = page.locator(".function-list-item", {
    has: page.locator(".function-list-item-name", { hasText: name }),
  });
  if ((await item.count()) > 0) {
    await item.first().click();
    await sleep(450);
    return;
  }
  const fallback = page.locator(".function-list-item").first();
  if (await fallback.count()) {
    await fallback.click();
    await sleep(450);
  }
}

async function waitWasm(page) {
  await page.waitForSelector(".rb-app", { timeout: 60000 });
  await page.waitForFunction(
    () => {
      const msg = document.body.textContent ?? "";
      if (msg.includes("WASM engine required for blast-radius")) return false;
      if (msg.includes("Waiting for WASM engine")) return false;
      return true;
    },
    { timeout: 90000 },
  );
  await sleep(1200);
}

async function waitForBlastResults(page) {
  await page.getByText("Callers of", { exact: false }).waitFor({ state: "visible", timeout: 25000 });
  await page.waitForFunction(
    () => {
      const el = document.querySelector(".blast-view .card-body .fs-4.fw-semibold.text-primary");
      return el && el.textContent && el.textContent.trim().length > 0;
    },
    { timeout: 25000 },
  );
  await sleep(400);
}

async function clearHighlights(page) {
  await page.evaluate(() => {
    document.querySelectorAll("[data-rb-demo-highlight]").forEach((el) => {
      el.style.outline = "";
      el.style.outlineOffset = "";
      el.style.boxShadow = "";
      el.removeAttribute("data-rb-demo-highlight");
    });
    document.getElementById("rb-demo-caption")?.remove();
  });
}

/** Highlight active tab button + main panel for HOLD_MS (no caption overlay). */
async function focusTabAndPanel(page, tabLabel, panelSelector) {
  if (tabLabel) {
    await clickTab(page, tabLabel);
  }

  await page.evaluate(
    ({ tabLabel, panelSelector }) => {
      const styleHighlight = (el) => {
        el.setAttribute("data-rb-demo-highlight", "1");
        el.style.outline = "3px solid #0d6efd";
        el.style.outlineOffset = "3px";
        el.style.boxShadow = "0 0 0 6px rgba(13, 110, 253, 0.15)";
      };

      const tabBar = document.querySelector(".rb-main-tabs");
      if (tabBar) {
        styleHighlight(tabBar);
        tabBar.scrollIntoView({ block: "nearest", behavior: "instant" });
      }

      if (tabLabel) {
        for (const btn of document.querySelectorAll(".rb-main-tabs .nav-link")) {
          const label = btn.querySelector("span")?.textContent?.trim() ?? btn.textContent?.trim();
          if (label === tabLabel) {
            styleHighlight(btn);
          }
        }
      }

      const workspace = document.querySelector(".rb-tab-workspace");
      if (workspace) styleHighlight(workspace);

      const panelCard = document.querySelector(".rb-tab-panel-card");
      if (panelCard) styleHighlight(panelCard);

      for (const sel of panelSelector.split(",").map((s) => s.trim())) {
        const panel = document.querySelector(sel);
        if (panel) {
          panel.scrollIntoView({ block: "nearest", behavior: "instant" });
          styleHighlight(panel);
          break;
        }
      }
    },
    { tabLabel, panelSelector },
  );

  await sleep(HOLD_MS);
  await clearHighlights(page);
}

async function prepareSegment(page, key) {
  try {
    switch (key) {
      case "graph-metrics": {
        const prBtn = page.getByRole("button", { name: /Sort by PR/i });
        if (await prBtn.count()) await prBtn.click();
        await sleep(300);
        break;
      }
      case "cfg": {
        await selectFunction(page, FN_CFG);
        const loadCfg = page.getByRole("button", { name: /Load CFG graph/i });
        if (await loadCfg.count()) await loadCfg.click();
        await page.locator(".cfg-detail").first().waitFor({ state: "visible", timeout: 25000 }).catch(() => {});
        await sleep(600);
        break;
      }
      case "pdg": {
        await selectFunction(page, FN);
        const dfView = page.locator("#df-view");
        if (await dfView.count()) {
          await dfView.selectOption("dataflow");
          await page.locator(".dataflow-graph-panel").waitFor({ state: "visible", timeout: 20000 }).catch(() => {});
        }
        await sleep(500);
        break;
      }
      case "dominance": {
        const dfView = page.locator("#df-view");
        if (await dfView.count()) {
          await dfView.selectOption("dominator");
          await sleep(700);
        }
        break;
      }
      case "program-slicing": {
        await selectFunction(page, FN_SLICE);
        await page.locator("#slice-line").fill(String(SLICE_LINE));
        await page.locator("#slice-var").fill(SLICE_VAR);
        await page.getByRole("button", { name: "Compute slice" }).click();
        await page.getByText(/slice:/i).waitFor({ state: "visible", timeout: 15000 }).catch(() => {});
        await sleep(400);
        break;
      }
      case "blast-radius": {
        await waitWasm(page);
        await selectFunction(page, FN_BLAST);
        await waitForBlastResults(page);
        break;
      }
      case "taint": {
        await selectFunction(page, FN_TAINT);
        await page.locator(".taint-view table tbody tr").first().waitFor({ state: "visible", timeout: 15000 }).catch(() => {});
        await page.locator(".taint-view table tbody tr").first().click().catch(() => {});
        await sleep(350);
        break;
      }
      case "migration": {
        await page.waitForSelector(".migration-tuning, .migration-view", { timeout: 20000 }).catch(() => {});
        await sleep(400);
        break;
      }
      case "semantic-search": {
        const input = page.locator('.search-view input[type="search"]');
        await input.waitFor({ state: "visible", timeout: 15000 });
        if (await input.isEnabled()) {
          await input.fill(SEMANTIC_QUERY);
          await page.locator('.search-view button[type="submit"]').click();
          await page.locator(".search-results tbody tr").first().waitFor({ state: "visible", timeout: 30000 }).catch(() => {});
          await sleep(400);
        }
        break;
      }
      case "ci-policy": {
        const section = page.locator(".guide-view section", { hasText: "Blast radius" });
        if (await section.count()) await section.first().scrollIntoViewIfNeeded();
        await sleep(300);
        break;
      }
      case "export": {
        const section = page.locator(".guide-view section", { hasText: "Graph visualization" });
        if (await section.count()) await section.first().scrollIntoViewIfNeeded();
        await sleep(300);
        break;
      }
      default:
        break;
    }
  } catch (err) {
    console.warn(`prepareSegment(${key}) skipped:`, err.message ?? err);
  }
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1280, height: 720 },
  recordVideo: { dir: OUT_DIR, size: { width: 1280, height: 720 } },
});
const page = await context.newPage();

await page.goto(BASE, { waitUntil: "networkidle", timeout: 120000 });
await waitWasm(page);

for (const segment of FEATURE_SEGMENTS) {
  if (segment.tab) {
    await clickTab(page, segment.tab);
  }
  await prepareSegment(page, segment.key);
  await focusTabAndPanel(page, segment.tab, segment.panel);
}

await clearHighlights(page);

const video = page.video();
await context.close();
await browser.close();

if (!video) throw new Error("Playwright did not return a video handle");

const saved = await video.path();
fs.renameSync(saved, RAW_WEBM);

writeSrt(OUT_SRT, FEATURE_SEGMENTS, SEC_PER_FEATURE);

const ffmpegBin = resolveFfmpeg();
const probe = spawnSync(
  "ffprobe",
  ["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", RAW_WEBM],
  { encoding: "utf8" },
);
const rawDur = parseFloat(probe.stdout.trim() || "0");

let vf = "fps=30,scale=1280:720:flags=lanczos";
let encodeMode = "native";
if (rawDur > TARGET_SECS + 1) {
  const factor = rawDur / TARGET_SECS;
  vf = `setpts=PTS/${factor},fps=30,scale=1280:720:flags=lanczos`;
  encodeMode = `speedup_${factor.toFixed(2)}x`;
}

const ffArgs = [
  "-y",
  "-i",
  RAW_WEBM,
  "-vf",
  vf,
  "-c:v",
  "libx264",
  "-preset",
  "fast",
  "-crf",
  "22",
  "-pix_fmt",
  "yuv420p",
  "-movflags",
  "+faststart",
];
if (rawDur > TARGET_SECS + 1) {
  ffArgs.push("-t", String(TARGET_SECS));
}

const ff = spawnSync(ffmpegBin, [...ffArgs, OUT_NO_CAPTIONS], { encoding: "utf8" });
if (ff.status !== 0) {
  console.error(ff.stderr);
  throw new Error("ffmpeg encode failed");
}

const finalProbe = spawnSync(
  "ffprobe",
  ["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", OUT_NO_CAPTIONS],
  { encoding: "utf8" },
);

console.log(
  JSON.stringify(
    {
      dashboard: BASE,
      output_no_captions: OUT_NO_CAPTIONS,
      srt: OUT_SRT,
      raw_duration_s: rawDur,
      final_duration_s: parseFloat(finalProbe.stdout.trim() || "0"),
      sec_per_feature: SEC_PER_FEATURE,
      target_secs: TARGET_SECS,
      encode_mode: encodeMode,
      features: FEATURE_SEGMENTS.map((s) => s.key),
      next: "./docs/videos/burn-feature-demo-captions.sh",
    },
    null,
    2,
  ),
);
