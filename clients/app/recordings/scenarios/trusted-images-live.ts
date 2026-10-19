import type { Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A trusted sender's images follow every change to the rule
// (campbooks-cloud#236). Needs only the Vite dev server: the browser's /bff
// calls, the change feed included, are answered here with a fictional
// workspace, and the server's part is played by this file (the real one is
// the conversations module's tests).
//
//   pnpm --dir clients/app exec vite --port 5236
//   pnpm --dir clients record trusted-images-live --base-url http://localhost:5236
//
// Sam trusts the Weekly Ledger's images: the address choice says what the
// checks vouch for (its domain). The rule is removed on another device: the
// open newsletter holds its images back again, live. Trusted again, he leaves
// it, and the rule is removed elsewhere once more: opened again, the
// newsletter waits for a fresh copy (its skeleton) and never loads its banner
// from the copy the app kept. Fictional people and mail only; nothing is sent.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const CONVERSATION = "0192f1a0-0000-7000-8000-0000000000c2";
const MESSAGE = "0192f1a0-0000-7000-8000-0000000000e4";
const RULE = "0192f1a0-0000-7000-8000-0000000000f2";
const SAM = "0192f1a0-0000-7000-8000-0000000000a1";
const API = `/bff/api/v1/w/${WORKSPACE}`;
const SUBJECT = "Five tax dates before year end";
const LEDGER = { name: "The Weekly Ledger", address: "hello@weekly-ledger.example" };
const BANNER = "https://cdn.weekly-ledger.example/banner.svg";
/** The server's placeholder for a held-back image. */
const PLACEHOLDER =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='560' height='120'%3E%3Crect width='560' height='120' fill='%23f1f0ee'/%3E%3C/svg%3E";

/** What the "server" holds: whether the rule trusts the sender, and the feed's frames. */
const state = {
  trusted: false,
  /** Conditions of the rule as saved. */
  conditions: [] as unknown[],
  frames: [] as string[],
  cursor: 1,
  /** How many times the banner was asked for: what a tracking image would count. */
  bannerLoads: 0,
};

const summary = {
  id: CONVERSATION,
  inbox_id: INBOX,
  subject: SUBJECT,
  stream: "primary",
  stream_source: "default",
  last_message_at: "2026-10-12T09:00:00Z",
  message_count: 1,
  unread_count: 0,
  starred: false,
  archived: false,
  trashed: false,
  spam: false,
  last_sent_at: null,
  sent_to: [],
  replied_since_sent: false,
  labels: [],
  suggestion: null,
  latest_from_you: false,
  latest_from: LEDGER,
  snippet: "Five tax dates before year end, plus what changes for freelancers in 2027.",
};

const detail = {
  conversation: summary,
  messages: [
    {
      id: MESSAGE,
      direction: "inbound",
      from: LEDGER,
      to: [{ name: "Sam Rivera", address: "sam@atelier-norte.example" }],
      cc: [],
      reply_to: [],
      subject: SUBJECT,
      sent_at: summary.last_message_at,
      read: true,
      starred: false,
      text: { content: summary.snippet, truncated: false },
      has_html: true,
      attachments: [],
    },
  ],
  decisions: [],
};

/** The newsletter's body as the server sanitises it: its banner's source only when shown. */
function body(asked: boolean) {
  const shown = asked || state.trusted;
  const banner = shown
    ? `<img src="${BANNER}" alt="The Weekly Ledger" width="560" height="120">`
    : `<img src="${PLACEHOLDER}" alt="The Weekly Ledger" width="560" height="120">`;
  return {
    text: summary.snippet,
    html: `${banner}<h1 style="font-size: 24px">${SUBJECT}</h1><p>Mark these in your calendar, and read what changes in 2027.</p>`,
    truncated: false,
    remote_images: 1,
    images_shown: shown,
    images_trusted: state.trusted,
    images_offer: !shown,
    images_domain: shown ? null : "weekly-ledger.example",
  };
}

function ruleView(removed = false) {
  return {
    id: RULE,
    inbox_id: INBOX,
    teaching_id: null,
    conditions: state.conditions,
    actions: [{ action: "show_images" }],
    origin: "action",
    origin_text: null,
    state: removed ? "removed" : "active",
    taught_by: SAM,
    from_conversation_id: CONVERSATION,
    created_at: "2026-10-19T09:00:00Z",
    confirmed_at: "2026-10-19T09:00:00Z",
    applied_count: 0,
    last_applied_at: null,
    recent_exceptions: 0,
    run: null,
    shared: false,
    may_change: true,
  };
}

/** The rule removed on another device: the server's state and its live change. */
function removeElsewhere() {
  state.trusted = false;
  state.cursor += 1;
  const cursor = String(state.cursor);
  const data = JSON.stringify({
    cursor,
    record: { type: "conversations.rule", id: RULE },
    op: "upsert",
    version: cursor,
  });
  state.frames.push(`id: ${cursor}\nevent: change\ndata: ${data}\n\n`);
}

async function answer(route: Route): Promise<void> {
  const request = route.request();
  const url = new URL(request.url());
  const path = url.pathname;
  const method = request.method();
  const json = (payload: unknown, status = 200) =>
    route.fulfill({
      status,
      contentType: status >= 400 ? "application/problem+json" : "application/json",
      body: JSON.stringify(payload),
    });
  if (path === "/bff/session") {
    return json({
      authenticated: true,
      user: { sub: "demo-account", name: "Sam Rivera", email: "sam.rivera@example.com" },
      expires_at: "2099-01-01T00:00:00Z",
    });
  }
  if (path === "/bff/api/v1/workspaces") {
    return json({
      workspaces: [
        {
          id: WORKSPACE,
          name: "Atelier Norte",
          kind: "business",
          accent: "business",
          role: "owner",
          created_at: "2026-09-01T09:00:00Z",
        },
      ],
    });
  }
  if (path.endsWith("/changes")) {
    // The change feed, as server-sent events: what changed since, then the
    // connection ends and `EventSource` comes back a second later.
    const frames = state.frames.splice(0);
    return route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: `retry: 1000\n\nevent: ready\ndata: {"cursor":"${String(state.cursor)}"}\n\n${frames.join("")}`,
    });
  }
  if (path === `${API}/members`) {
    return json({
      members: [
        {
          user_id: SAM,
          display_name: "Sam Rivera",
          email: "sam.rivera@example.com",
          role: "owner",
          joined_at: "2026-09-01T09:00:00Z",
          you: true,
        },
      ],
    });
  }
  if (path === `${API}/inboxes`) {
    return json({
      inboxes: [
        {
          id: INBOX,
          channel: "email",
          address: "sam@atelier-norte.example",
          display_name: "Sam Rivera",
          role: "manager",
          read_only: false,
          unread_count: 0,
        },
      ],
    });
  }
  if (path === `${API}/streams`) {
    return json({
      streams: ["primary", "updates", "receipts", "newsletters"].map((stream) => ({
        stream,
        conversations: stream === "primary" ? 1 : 0,
        unread: 0,
      })),
    });
  }
  if (path === `${API}/conversations`) {
    const primary = url.searchParams.get("stream") === "primary";
    return json({ conversations: primary ? [summary] : [], next_cursor: null });
  }
  if (path === `${API}/conversations/${CONVERSATION}`) {
    return json(detail);
  }
  if (path === `${API}/conversations/${CONVERSATION}/messages/${MESSAGE}/body`) {
    // A moment's wait, as a real read takes: the reader's skeleton shows.
    await pause(900);
    return json(body(url.searchParams.get("show_images") === "true"));
  }
  if (path === `${API}/conversations/${CONVERSATION}/read`) {
    return json(summary);
  }
  if (path === `${API}/conversations/${CONVERSATION}/rule-suggestions`) {
    return json({ shared: false, default: 0, none: null, suggestions: [] });
  }
  if (path === `${API}/inboxes/${INBOX}/rules/preview`) {
    return json({
      matching: 3,
      would_change: 0,
      samples: [],
      skipped_suspicious: 0,
      skipped_member: 0,
      overrides: [],
      complete: true,
    });
  }
  if (path === `${API}/inboxes/${INBOX}/rules` && method === "POST") {
    const sent = (request.postDataJSON() ?? {}) as { conditions?: unknown[] };
    state.conditions = sent.conditions ?? [];
    state.trusted = true;
    return json({ rule: ruleView(), acted_without_asking: false }, 201);
  }
  if (path === `${API}/rules`) {
    return json({ rules: state.trusted ? [ruleView()] : [] });
  }
  if (path === `${API}/rules/${RULE}` && method === "DELETE") {
    state.trusted = false;
    return json(ruleView(true));
  }
  if (path === `${API}/rules/${RULE}`) {
    return json(ruleView(!state.trusted));
  }
  if (path === `${API}/labels`) {
    return json({ labels: [] });
  }
  if (path === `${API}/drafts`) {
    return json({ drafts: [] });
  }
  if (path === `${API}/assistant/profile`) {
    return json({ proactivity: "ask_first", asked_at: "2026-10-01T09:00:00Z", revision: 1 });
  }
  if (path === `${API}/attention`) {
    return json({ items: [], counts: [] });
  }
  if (path === `${API}/rule-notices`) {
    return json({ notices: [] });
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

/** The banner, when the frame is let to ask for it: counted, as a tracker would. */
async function banner(route: Route): Promise<void> {
  state.bannerLoads += 1;
  await route.fulfill({
    status: 200,
    contentType: "image/svg+xml",
    body: `<svg xmlns="http://www.w3.org/2000/svg" width="560" height="120"><rect width="560" height="120" fill="#2f5d50"/><text x="24" y="72" fill="#fff" font-size="32" font-family="sans-serif">The Weekly Ledger</text></svg>`,
  });
}

export default async function trustedImagesLive({ page }: ScenarioContext): Promise<void> {
  Object.assign(state, { trusted: false, conditions: [], frames: [], cursor: 1, bannerLoads: 0 });
  await page.route("**/bff/**", answer);
  await page.route("https://cdn.weekly-ledger.example/**", banner);

  await page.goto(`/w/${WORKSPACE}/inbox/primary/${CONVERSATION}`);
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: SUBJECT }).waitFor();
  await reader
    .getByText(/remote image hidden/u)
    .first()
    .waitFor();
  await caption(page, "The Weekly Ledger's banner is held back", 1800);

  await reader.getByRole("button", { name: /^Always show images from The Weekly Ledger/u }).click();
  const dialog = page.getByRole("dialog", {
    name: "Always show images from hello@weekly-ledger.example",
  });
  await dialog.waitFor();
  await dialog.getByText(/vouch for the domain, not for this one person/u).waitFor();
  await caption(page, "Trusting one address: what the checks vouch for is its domain", 3000);
  await dialog.getByRole("button", { name: /Save smart rule/u }).click();
  await dialog.waitFor({ state: "hidden" });
  await reader.getByText(/Images shown: you trust The Weekly Ledger/u).waitFor();
  await caption(page, "Trusted: the banner shows", 2000);

  // Removed on another device: the server's state, and its live change.
  await caption(page, "Another device removes the rule…", 1600);
  removeElsewhere();
  await reader.getByRole("button", { name: /^Show images/u }).waitFor({ timeout: 20_000 });
  await caption(page, "…and the open newsletter holds its banner back again, live", 2800);

  // Trusted again, then left; the rule removed elsewhere while it is closed.
  await reader.getByRole("button", { name: /^Always show images from The Weekly Ledger/u }).click();
  await dialog.waitFor();
  await dialog.getByRole("button", { name: /Save smart rule/u }).click();
  await dialog.waitFor({ state: "hidden" });
  await reader.getByText(/Images shown: you trust The Weekly Ledger/u).waitFor();
  await caption(page, "Trusted again. Sam leaves the newsletter", 1800);
  await page.keyboard.press("Escape");
  await pause(900);
  await caption(page, "While it is closed, the rule is removed on another device", 2200);
  removeElsewhere();
  await pause(2500);
  const loadsBefore = state.bannerLoads;

  await page
    .getByRole("button", { name: /The Weekly Ledger/u })
    .first()
    .click();
  await caption(page, "Opened again: it waits for a fresh copy, not the one kept", 1200);
  await reader.getByRole("button", { name: /^Show images/u }).waitFor({ timeout: 20_000 });
  await caption(
    page,
    state.bannerLoads === loadsBefore
      ? "Held back: the banner was never asked for again"
      : "The banner was asked for again: this is the bug",
    3000,
  );
}
