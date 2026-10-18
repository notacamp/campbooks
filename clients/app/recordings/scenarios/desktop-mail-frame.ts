import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { Page } from "playwright";

import { frameDocument, SANDBOX } from "../../src/lib/ui/safe-html/frame.ts";
import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// HTML mail under the desktop app's page policy (campbooks-cloud#199), in
// WebKit, the desktop engine on macOS and Linux. The page is served with the
// policy `src-tauri/tauri.conf.json` gives the app, around the frame the
// reader renders (`lib/ui/safe-html/frame.ts`) and the server's own output
// for the #160 fixtures (`test/webkit/fixtures`). Nothing leaves the machine:
// demo images are answered here. Start any dev server for the base URL, then:
//
//   pnpm --filter @campbooks/app exec playwright install webkit
//   pnpm --dir clients record desktop-mail-frame --browser webkit
//
// Locators are scoped to the page's header and the named frame.

const app = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (file: string) => readFileSync(path.join(app, file), "utf8");

const POLICY = (
  JSON.parse(read("src-tauri/tauri.conf.json")) as { app: { security: { csp: string } } }
).app.security.csp;

const FIXTURES = [
  ["marketing-600", "A 600px marketing layout"],
  ["newsletter", "A newsletter"],
  ["outlook", "Mail written in Outlook"],
] as const;

/** A demo picture for every remote image, so "Show images" shows something. */
const DEMO_IMAGE = `<svg xmlns="http://www.w3.org/2000/svg" width="600" height="240" viewBox="0 0 600 240"><defs><linearGradient id="g" x1="0" x2="1"><stop offset="0" stop-color="#2f5d50"/><stop offset="1" stop-color="#e85d04"/></linearGradient></defs><rect width="600" height="240" fill="url(#g)"/><text x="300" y="132" font-family="Georgia, serif" font-size="28" fill="#ffffff" text-anchor="middle">Demo image</text></svg>`;

/** The page: a header saying what it shows, and the mail frame. */
function screen(fixture: string, label: string, images: boolean): string {
  const html = read(`test/webkit/fixtures/${fixture}.${images ? "shown" : "held"}.html`);
  const srcdoc = frameDocument(html, images).replaceAll("&", "&amp;").replaceAll('"', "&quot;");
  const show = images
    ? `<span style="color: #2f5d50; font-weight: 600">Images shown · tracking pixels removed</span>`
    : `<a href="?images=1" style="color: #ffffff; background: #1f2328; padding: 8px 14px; border-radius: 8px; text-decoration: none; font-weight: 600">Show images</a>`;
  return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Desktop policy</title></head>
<body style="margin: 0; background: #f6f5f3; font: 15px/1.5 -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif; color: #1f2328">
<header role="banner" style="display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: space-between; padding: 16px; border-bottom: 1px solid #e3e1dd; background: #ffffff">
<div><strong>${label}</strong><div style="color: #6b6965; font-size: 13px">The desktop app's page policy · WebKit</div></div>${show}</header>
<main style="padding: 16px"><iframe title="Message" sandbox="${SANDBOX}" referrerpolicy="no-referrer" style="display: block; width: 100%; max-width: 760px; height: 1400px; margin: 0 auto; border: 0; border-radius: 12px; background: #ffffff" srcdoc="${srcdoc}"></iframe></main>
</body></html>`;
}

async function serve(tab: Page): Promise<void> {
  await tab.route(/\/__desktop-mail\//u, (route) => {
    const url = new URL(route.request().url());
    const fixture = url.pathname.split("/").at(-1) ?? "";
    const label = FIXTURES.find(([name]) => name === fixture)?.[1] ?? fixture;
    return route.fulfill({
      status: 200,
      headers: {
        "content-type": "text/html; charset=utf-8",
        "content-security-policy": POLICY,
      },
      body: screen(fixture, label, url.searchParams.get("images") === "1"),
    });
  });
  await tab.route(/^https:\/\/(?!localhost|127\.0\.0\.1)/u, (route) =>
    route.fulfill({ status: 200, contentType: "image/svg+xml", body: DEMO_IMAGE }),
  );
}

export default async function desktopMailFrame({ page: tab }: ScenarioContext): Promise<void> {
  await serve(tab);
  for (const [fixture, label] of FIXTURES) {
    await tab.goto(`/__desktop-mail/${fixture}`);
    await tab.getByRole("banner").getByText(label).waitFor();
    await caption(tab, `${label}, under the desktop app's policy: styled, images held back`, 2600);
    await tab.getByRole("banner").getByRole("link", { name: "Show images" }).click();
    await tab
      .getByRole("banner")
      .getByText(/Images shown/u)
      .waitFor();
    await pause(800);
    await caption(tab, "Show images: https images load; tracking pixels stay removed", 2400);
    await pause(1200);
  }
}
