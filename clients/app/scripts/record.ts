// Records a scenario at 375px (mobile) and desktop widths as .webm files.
// Usage: pnpm --dir clients record <scenario> [--base-url http://localhost:5173]
// The base URL must be on this machine (localhost, 127.0.0.1, [::1]): a
// recording is never made against a deployed instance. `--allow-remote`
// overrides that, for a disposable instance holding demo data only.
import { existsSync, readdirSync, mkdirSync, renameSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { chromium, type BrowserContextOptions } from "playwright";
import type { ScenarioContext } from "../recordings/context.ts";
import { refuseTarget } from "./record-target.ts";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "recordings");
const scenariosDir = path.join(root, "scenarios");
const outDir = path.join(root, "out");

const profiles: { name: string; options: BrowserContextOptions }[] = [
  {
    name: "375",
    options: {
      viewport: { width: 375, height: 812 },
      isMobile: true,
      hasTouch: true,
      deviceScaleFactor: 2,
    },
  },
  { name: "desktop", options: { viewport: { width: 1440, height: 900 } } },
];

function fail(message: string): never {
  throw new Error(message);
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      "base-url": { type: "string", default: "http://localhost:5173" },
      "allow-remote": { type: "boolean", default: false },
    },
  });
  const baseURL = values["base-url"];
  const refused = refuseTarget(baseURL, values["allow-remote"]);
  if (refused !== undefined) fail(refused);
  const name = positionals[0];
  const available = readdirSync(scenariosDir)
    .filter((file) => file.endsWith(".ts"))
    .map((file) => file.slice(0, -3));

  if (!name) fail(`missing scenario. Available: ${available.join(", ")}`);
  const file = path.join(scenariosDir, `${name}.ts`);
  if (!existsSync(file)) fail(`no scenario "${name}". Available: ${available.join(", ")}`);

  try {
    await fetch(baseURL, { signal: AbortSignal.timeout(5000) });
  } catch {
    fail(`cannot reach ${baseURL}. Start the dev server: pnpm --dir clients dev`);
  }

  const scenario = (await import(file)) as { default: (ctx: ScenarioContext) => Promise<void> };
  mkdirSync(outDir, { recursive: true });
  const browser = await chromium.launch();
  const written: string[] = [];
  try {
    for (const { name: profile, options } of profiles) {
      const viewport = options.viewport ?? { width: 1440, height: 900 };
      const context = await browser.newContext({
        ...options,
        baseURL,
        recordVideo: { dir: outDir, size: viewport },
      });
      const page = await context.newPage();
      const video = page.video();
      try {
        await scenario.default({ page });
      } finally {
        await context.close();
      }
      const target = path.join(outDir, `${name}-${profile}.webm`);
      if (video) renameSync(await video.path(), target);
      written.push(target);
    }
  } finally {
    await browser.close();
  }

  console.log(`\nRecorded:\n${written.map((file) => `  ${file}`).join("\n")}\n`);
  console.log("Next: maintainers run `upload-media <file>`; contributors drag the file");
  console.log('into the PR description\'s "## Recording" section.');
}

try {
  await main();
} catch (error: unknown) {
  console.error(`record: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
