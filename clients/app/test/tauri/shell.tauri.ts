// The mail frame in the real desktop shell (campbooks-cloud#237, phase 2 of
// #199): a debug build of the Tauri app, driven by `tauri-driver` and
// WebKitWebDriver under a virtual display, pointed at a mock API serving one
// conversation whose HTML mail is hostile (markup the server's sanitiser
// removes, kept on purpose: the frame must hold on its own). Inside the mail
// frame's realm it checks that no Tauri IPC object exists, that none of the
// mail's scripts ran and no invoke reached the API, that inline styles apply
// and nothing remote loads, that a `target=_blank` link opens no webview, and
// that a meta refresh or a nested iframe never reaches `open_outside` (#550:
// on macOS wry's navigation policy has no main-frame check).
//
// The debug build records every link handed to `open_outside` in
// `CAMPBOOKS_DEV_OPEN_OUTSIDE_LOG` instead of opening it (release builds never
// read it). Run it as CI's `tauri-shell` job does:
//
//   pnpm --filter @campbooks/app tauri build --debug --no-bundle
//   cargo install tauri-driver --locked   # and webkit2gtk-driver, xvfb
//   xvfb-run -a pnpm --filter @campbooks/app test:tauri
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const BINARY =
  process.env["CAMPBOOKS_SHELL_BINARY"] ??
  path.join(here, "../../src-tauri/target/debug/campbooks-app");
const DRIVER = "http://127.0.0.1:4444";

// One fictional workspace, inbox and conversation, as the API returns them.
const WORKSPACE = {
  id: "0192f1a0-0000-7000-8000-000000000001",
  name: "Duarte & Pessoa",
  kind: "business",
  accent: "business",
  role: "owner",
  created_at: "2026-09-01T09:00:00Z",
};
const INBOX = {
  id: "0192f1a0-0000-7000-8000-0000000000b1",
  channel: "email",
  address: "sam.rivera@example.com",
  display_name: "Sam Rivera",
  role: "manager",
  read_only: false,
  unread_count: 1,
};
const SENDER = { name: "Quay Office", address: "office@quay-moorings.example" };
const CONVERSATION = {
  id: "0192f1a0-0000-7000-8000-0000000000c2",
  inbox_id: INBOX.id,
  archived: false,
  labels: [],
  suggestion: null,
  subject: "Your berth for next season",
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-12T09:00:00Z",
  message_count: 1,
  unread_count: 0,
  starred: false,
  latest_from: SENDER,
  snippet: "Your berth is kept for next season.",
};
const MESSAGE_ID = "0192f1a0-0000-7000-8000-0000000000e4";
const DETAIL = {
  conversation: CONVERSATION,
  messages: [
    {
      id: MESSAGE_ID,
      direction: "inbound",
      from: SENDER,
      to: [{ name: "Sam Rivera", address: INBOX.address }],
      cc: [],
      reply_to: [],
      subject: CONVERSATION.subject,
      sent_at: CONVERSATION.last_message_at,
      read: true,
      starred: false,
      text: { content: CONVERSATION.snippet, truncated: false },
      has_html: true,
      attachments: [],
    },
  ],
  decisions: [],
};

/** Where the mail's own attempts would land: the mock API, which records them. */
let api = "";

/** Markup no sanitiser would let through, aimed at the shell's IPC and the system opener. */
function hostile(): string {
  const invoke = `parent.__TAURI_INTERNALS__ && parent.__TAURI_INTERNALS__.invoke("api_request", { method: "GET", path: "/api/v1/pwned" })`;
  return [
    `<p data-probe style="color: rgb(200, 0, 0); font-weight: 700">Styled by its own attribute</p>`,
    `<img src="${api}/remote.png" width="40" height="20" alt="plain http">`,
    `<img src="https://images.example/banner.png" width="40" height="20" alt="remote">`,
    `<script>parent.pwned = "script"; ${invoke};</script>`,
    `<img src="data:," onerror='parent.pwned = "onerror"; ${invoke}' alt="">`,
    `<svg onload='parent.pwned = "svg"; ${invoke}'><rect width="1" height="1"></rect></svg>`,
    `<a id="blank" href="https://outside.example/blank" target="_blank">a link that asks for a new window</a>`,
    `<meta http-equiv="refresh" content="0;url=https://outside.example/refresh">`,
    `<iframe src="https://outside.example/nested" width="40" height="20"></iframe>`,
  ].join("\n");
}

/** Every request the mock API received, as `METHOD /path`. */
const received: string[] = [];

function answer(method: string, pathname: string): unknown {
  const w = `/api/v1/w/${WORKSPACE.id}`;
  const routes: Record<string, unknown> = {
    "GET /api/v1/workspaces": { workspaces: [WORKSPACE] },
    [`GET ${w}/inboxes`]: { inboxes: [INBOX] },
    [`GET ${w}/streams`]: {
      streams: ["primary", "updates", "receipts", "newsletters"].map((stream) => ({
        stream,
        conversations: stream === "primary" ? 1 : 0,
        unread: 0,
      })),
    },
    [`GET ${w}/conversations`]: { conversations: [CONVERSATION], next_cursor: null },
    [`GET ${w}/conversations/${CONVERSATION.id}`]: DETAIL,
    [`GET ${w}/conversations/${CONVERSATION.id}/messages/${MESSAGE_ID}/body`]: {
      text: CONVERSATION.snippet,
      html: hostile(),
      truncated: false,
      remote_images: 2,
      images_shown: false,
      images_trusted: false,
      images_offer: false,
      images_domain: null,
    },
    [`GET ${w}/drafts`]: { drafts: [] },
  };
  return routes[`${method} ${pathname}`];
}

let server: Server;
let driver: ChildProcess;
let session = "";
let opened = "";

/** One WebDriver command; its `value`. */
async function command(method: string, url: string, body?: unknown): Promise<unknown> {
  const response = await fetch(`${DRIVER}${url}`, {
    method,
    headers: { "content-type": "application/json" },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const json = (await response.json()) as { value: unknown };
  if (!response.ok) {
    throw new Error(`${method} ${url}: ${JSON.stringify(json.value)}`);
  }
  return json.value;
}

/** Run `script` in the app's page (a script string: no DOM types here). */
function run(script: string, args: unknown[] = []): Promise<unknown> {
  return command("POST", `/session/${session}/execute/sync`, { script, args });
}

/** Wait until `check` holds, for up to `seconds`. */
async function until(
  what: string,
  check: () => Promise<boolean> | boolean,
  seconds = 60,
): Promise<void> {
  for (let tick = 0; tick < seconds * 4; tick += 1) {
    if (
      await Promise.resolve()
        .then(check)
        .catch(() => false)
    ) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timed out waiting for ${what}`);
}

const FRAME = `document.querySelector('iframe[title^="Message from"]')`;

beforeAll(async () => {
  if (!existsSync(BINARY)) {
    throw new Error(`no debug build at ${BINARY}: run \`pnpm tauri build --debug --no-bundle\``);
  }
  server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    received.push(`${request.method ?? "GET"} ${url.pathname}`);
    const body = answer(request.method ?? "GET", url.pathname);
    if (body === undefined) {
      response
        .writeHead(404, { "content-type": "application/problem+json" })
        .end(JSON.stringify({ title: "Not found", status: 404, code: "not_found" }));
      return;
    }
    response.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify(body));
  });
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", resolve);
  });
  api = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
  opened = path.join(mkdtempSync(path.join(tmpdir(), "campbooks-shell-")), "opened.log");
  driver = spawn("tauri-driver", ["--port", "4444"], {
    env: {
      ...process.env,
      CAMPBOOKS_API_URL: api,
      CAMPBOOKS_DEV_OPEN_OUTSIDE_LOG: opened,
    },
    stdio: "inherit",
  });
  await until(
    "tauri-driver",
    async () => {
      const status = await fetch(`${DRIVER}/status`);
      return status.ok;
    },
    30,
  );
  const created = (await command("POST", "/session", {
    capabilities: {
      alwaysMatch: { browserName: "wry", "tauri:options": { application: BINARY } },
    },
  })) as { sessionId: string };
  session = created.sessionId;
  // The app is up when its router has rendered something.
  await until("the app", async () => (await run("return document.readyState")) === "complete");
  // Open the conversation in the app's own router (no page load).
  await run(
    `history.pushState(null, "", arguments[0]); dispatchEvent(new PopStateEvent("popstate"));`,
    [`/w/${WORKSPACE.id}/inbox/primary/${CONVERSATION.id}`],
  );
  await until("the mail frame", async () => (await run(`return ${FRAME} !== null`)) === true, 90);
  // Late loads (a refresh, an image, a nested frame) get their chance.
  await new Promise((resolve) => setTimeout(resolve, 2000));
}, 180_000);

afterAll(async () => {
  if (session !== "") {
    await command("DELETE", `/session/${session}`).catch(() => null);
  }
  driver.kill();
  await new Promise((resolve) => server.close(resolve));
});

/** What the shell handed to `open_outside`. */
function handedOutside(): string[] {
  return existsSync(opened) ? readFileSync(opened, "utf8").split("\n").filter(Boolean) : [];
}

describe("the mail frame in the desktop shell", () => {
  it("has no Tauri IPC in its realm, while the app's own page does", async () => {
    const realms = (await run(`
      const frame = ${FRAME};
      return {
        page: typeof window.__TAURI_INTERNALS__,
        internals: typeof frame.contentWindow.__TAURI_INTERNALS__,
        tauri: typeof frame.contentWindow.__TAURI__,
        sandbox: frame.getAttribute("sandbox"),
      };
    `)) as Record<string, string>;
    // The probe would see it: the app's page has it.
    expect(realms["page"]).toBe("object");
    expect(realms["internals"]).toBe("undefined");
    expect(realms["tauri"]).toBe("undefined");
    expect(realms["sandbox"]).not.toContain("allow-scripts");
  });

  it("ran none of the mail's scripts, and nothing reached the API for it", async () => {
    expect(await run("return window.pwned === undefined ? null : String(window.pwned)")).toBeNull();
    expect(received.filter((request) => request.includes("pwned"))).toEqual([]);
  });

  it("applies inline styles and loads nothing remote before Show images", async () => {
    const color = await run(
      `const doc = ${FRAME}.contentDocument; return getComputedStyle(doc.querySelector("[data-probe]")).color;`,
    );
    expect(color).toBe("rgb(200, 0, 0)");
    expect(received.filter((request) => request.includes("remote.png"))).toEqual([]);
  });

  it("never hands a meta refresh or a nested frame to the system opener", async () => {
    // The frame is still the mail, and nothing of it went outside.
    expect(await run(`return ${FRAME}.contentWindow.location.href`)).toBe("about:srcdoc");
    const outside = handedOutside();
    expect(outside.filter((url) => url.includes("refresh") || url.includes("nested"))).toEqual([]);
  });

  it("opens no webview for a target=_blank link: it goes to the system opener alone", async () => {
    const frame = (await command("POST", `/session/${session}/element`, {
      using: "css selector",
      value: 'iframe[title^="Message from"]',
    })) as Record<string, string>;
    await command("POST", `/session/${session}/frame`, { id: frame });
    const link = (await command("POST", `/session/${session}/element`, {
      using: "css selector",
      value: "#blank",
    })) as Record<string, string>;
    const id = Object.values(link)[0] ?? "";
    await command("POST", `/session/${session}/element/${id}/click`, {});
    await command("POST", `/session/${session}/frame/parent`, {});
    await until("the link handed outside", () =>
      handedOutside().includes("https://outside.example/blank"),
    );
    const windows = (await command("GET", `/session/${session}/window/handles`)) as string[];
    expect(windows).toHaveLength(1);
    // Still nothing else: no refresh, no nested frame.
    expect(handedOutside()).toEqual(["https://outside.example/blank"]);
  });
});
