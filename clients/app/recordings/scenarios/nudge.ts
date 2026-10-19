import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Waiting on others and the nudge (campbooks-cloud#293). Needs only the Vite
// dev server: the browser's /bff calls, the change feed included, are
// answered here with a fictional workspace, and the server's part (the wait,
// the nudge draft, the send and hearing back) is played by this file (the
// real one is the conversations and attention modules' tests).
//
//   pnpm --dir clients/app exec vite --port 5293
//   pnpm --dir clients record nudge --base-url http://localhost:5293
//
// João Matos asked Ana Costa, copying Sam, for the revised floor plan six
// days ago; Ana hasn't written, so Home lists it under Nudge, after a reply
// and a bill. Sam presses 1: Scout writes a few lines to Ana, copying João;
// Shorter, then Send. Once the nudge leaves, Scout waits on Ana again
// (Waiting on others, beside Rui Matos's promise). Then Ana writes back and
// the wait ends: "Heard back" in Done today. Fictional people and mail only;
// nothing is sent.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const SAM = "0192f1a0-0000-7000-8000-0000000000a1";
const DRAFT = "0192f1a0-0000-7000-8000-0000000000d3";
const API = `/bff/api/v1/w/${WORKSPACE}`;
const ME = { name: "Sam Rivera", address: "sam@atelier-norte.example" };
const ANA_ADDRESS = { name: "Ana Costa", address: "ana@costa-arquitetos.example" };
const JOAO = { name: "João Matos", address: "joao@obras-matos.example" };

const TEXT =
  "Hi Ana,\n\nJust following up on João’s note below: could you send me the revised floor plan when you have a moment? I’d like to go through it before Thursday’s visit.\n\nThanks,\nSam Rivera";
const SHORT =
  "Hi Ana,\n\nCould you send me the revised floor plan before Thursday?\n\nThanks,\nSam";

const hex = (n: number) => n.toString(16).padStart(12, "0");
const conversationId = (n: number) => `0192f1a0-0000-7000-8000-${hex(0x10_00 + n)}`;
const messageId = (n: number) => `0192f1a0-0000-7000-8000-${hex(0x20_00 + n)}`;
const itemId = (n: number) => `0192f1a0-0000-7000-9000-${hex(0x10_00 + n)}`;

const DAY = 86_400_000;
const ago = (days: number, hours = 0) => new Date(Date.now() - days * DAY - hours * 3_600_000);
/** A calendar date, `days` from today. */
const dateIn = (days: number) => new Date(Date.now() + days * DAY).toISOString().slice(0, 10);
/** The next Thursday after today (when Scout checks again after the nudge). */
function nextThursday(): string {
  const days = (4 - new Date().getDay() + 7) % 7 || 7;
  return dateIn(days);
}

interface Spec {
  n: number;
  subject: string;
  from: { name: string; address: string };
  sent: Date;
  need: Record<string, unknown> | null;
}

function summary(spec: Spec) {
  return {
    id: conversationId(spec.n),
    inbox_id: INBOX,
    subject: spec.subject,
    stream: "primary",
    stream_source: "default",
    last_message_at: spec.sent.toISOString(),
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
    latest_from: spec.from,
    latest_from_you: false,
    snippet: "",
    need:
      spec.need === null
        ? null
        : {
            basis: "rules",
            overdue: false,
            urgent: false,
            rule: "model.decide",
            due_on: null,
            message_id: messageId(spec.n),
            sent_at: spec.sent.toISOString(),
            quote: null,
            ...spec.need,
          },
  };
}

/** What the "server" holds: Ana's wait, Done today, and the change feed's frames. */
const state = {
  /** `nudge` (on Home), `waiting` (nudged, Scout checks again) or `heard` (Ana wrote). */
  ana: "nudge" as "nudge" | "waiting" | "heard",
  frames: [] as string[],
  cursor: 1,
};

const MARTA = summary({
  n: 1,
  subject: "Walkthrough on Thursday",
  from: { name: "Marta Ferreira", address: "marta@casa-lume.example" },
  sent: ago(0, 3),
  need: {
    reason: "reply",
    trust: "sure",
    question: "needs_reply",
    rule: "signals.reply_asked",
    due_on: dateIn(1),
    quote: {
      text: "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
      from: { name: "Marta Ferreira", address: "marta@casa-lume.example" },
      sent_at: ago(0, 3).toISOString(),
    },
  },
});

const EDP = summary({
  n: 2,
  subject: "A sua fatura de outubro",
  from: { name: "EDP Comercial", address: "faturas@edp-comercial.example" },
  sent: ago(1),
  need: {
    reason: "pay",
    trust: "check",
    question: "money",
    due_on: dateIn(9),
  },
});

const RUI = summary({
  n: 4,
  subject: "Signed contract for the kitchen units",
  from: { name: "Rui Matos", address: "rui@matos-carpintaria.example" },
  sent: ago(1, 2),
  need: {
    reason: "waiting",
    trust: "sure",
    question: "next_move",
    due_on: dateIn(2),
    waiting_on: { name: "Rui Matos", address: "rui@matos-carpintaria.example" },
    nudges: 0,
  },
});

const ANA_SENT = ago(6);

function ana() {
  const need =
    state.ana === "heard"
      ? null
      : {
          reason: state.ana,
          trust: "sure",
          question: "next_move",
          due_on: state.ana === "nudge" ? dateIn(-1) : nextThursday(),
          waiting_on: ANA_ADDRESS,
          nudges: state.ana === "nudge" ? 0 : 1,
          quote: {
            text: "Ana, could you send Sam the revised floor plan directly, before Thursday’s visit?",
            from: JOAO,
            sent_at: ANA_SENT.toISOString(),
          },
        };
  return summary({ n: 3, subject: "Revised floor plan", from: JOAO, sent: ANA_SENT, need });
}

function anaDetail() {
  return {
    conversation: ana(),
    messages: [
      {
        id: messageId(3),
        direction: "inbound",
        from: JOAO,
        to: [ANA_ADDRESS],
        cc: [ME],
        reply_to: [],
        subject: "Revised floor plan",
        sent_at: ANA_SENT.toISOString(),
        read: true,
        starred: false,
        text: {
          content:
            "Ana,\n\nCould you send Sam the revised floor plan directly, with the kitchen on the north wall, before Thursday’s visit?\n\nThanks,\nJoão",
          truncated: false,
        },
        has_html: false,
        attachments: [],
      },
    ],
    decisions: [],
    need_summary:
      "João asked Ana Costa to send you the revised floor plan before Thursday’s visit. Nothing has come from Ana since.",
  };
}

/** An attention item for `conversation`, as `attention.list` returns it. */
function item(n: number, conversation: ReturnType<typeof summary>, extra = {}) {
  const need = conversation.need as { reason?: unknown; trust?: unknown; due_on?: unknown } | null;
  return {
    id: itemId(n),
    record: { type: "conversations.conversation", id: conversation.id },
    container: { type: "conversations.inbox", id: INBOX },
    cause: { type: "conversations.message", id: messageId(n) },
    reason: need?.reason ?? "reply",
    trust: need?.trust ?? "sure",
    due_on: need?.due_on ?? null,
    overdue: false,
    urgent: false,
    since: conversation.last_message_at,
    state: "open",
    cleared_because: null,
    cleared_by: null,
    cleared_at: null,
    snoozed_until: null,
    ...extra,
  };
}

function attention(which: string | null) {
  if (which === "cleared") {
    return state.ana === "heard"
      ? [
          item(3, ana(), {
            reason: "waiting",
            state: "cleared",
            cleared_because: "heard_back",
            cleared_at: new Date().toISOString(),
          }),
        ]
      : [];
  }
  if (which === "waiting") {
    const rui = item(4, RUI);
    // Soonest check first: Rui (in two days), then Ana (Thursday).
    return state.ana === "waiting" ? [rui, item(3, ana())] : [rui];
  }
  return state.ana === "nudge"
    ? [item(1, MARTA), item(2, EDP), item(3, ana())]
    : [item(1, MARTA), item(2, EDP)];
}

/** Ana's wait changed on the "server": its live change, for the attention list and the row. */
function changed() {
  for (const record of [
    { type: "attention.item", id: itemId(3) },
    { type: "conversations.conversation", id: conversationId(3) },
  ]) {
    state.cursor += 1;
    const cursor = String(state.cursor);
    const data = JSON.stringify({ cursor, record, op: "upsert", version: cursor });
    state.frames.push(`id: ${cursor}\nevent: change\ndata: ${data}\n\n`);
  }
}

function draftView(body: string, scheduled: boolean) {
  return {
    id: DRAFT,
    inbox_id: INBOX,
    kind: "nudge",
    conversation_id: conversationId(3),
    message_id: messageId(3),
    from: ME,
    to: [ANA_ADDRESS],
    cc: [JOAO],
    bcc: [],
    subject: "Re: Revised floor plan",
    body,
    quoted: "",
    attachments: [],
    version: 1,
    state: scheduled ? "scheduled" : "draft",
    send_at: scheduled ? new Date(Date.now() + 10_000).toISOString() : null,
    failure_reason: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    content_token: "demo-token-nudge",
  };
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
  const sent = (request.postDataJSON() ?? {}) as Record<string, unknown>;
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
          address: ME.address,
          display_name: ME.name,
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
        conversations: 0,
        unread: 0,
      })),
    });
  }
  if (path === `${API}/attention`) {
    return json({ items: attention(url.searchParams.get("state")), counts: [] });
  }
  if (path === `${API}/conversations`) {
    const ids = new Set((url.searchParams.get("ids") ?? "").split(",").filter(Boolean));
    const all = [MARTA, EDP, ana(), RUI];
    return json({
      conversations: ids.size === 0 ? all : all.filter((c) => ids.has(c.id)),
      next_cursor: null,
    });
  }
  if (path === `${API}/conversations/${conversationId(3)}`) {
    return json(anaDetail());
  }
  if (path === `${API}/conversations/${conversationId(3)}/nudge/draft` && method === "POST") {
    // Scout's canned, fictional nudge, or it shorter.
    await pause(1200);
    return json({ message_id: messageId(3), text: sent["shorter"] === true ? SHORT : TEXT });
  }
  if (path === `${API}/drafts` && method === "POST") {
    return json(draftView(typeof sent["body"] === "string" ? sent["body"] : "", false), 201);
  }
  if (path === `${API}/drafts/${DRAFT}/send` && method === "POST") {
    return json(draftView(SHORT, true), 202);
  }
  if (path === `${API}/drafts/${DRAFT}/cancel_send` && method === "POST") {
    return json(draftView(SHORT, false));
  }
  if (path === `${API}/drafts`) {
    return json({ drafts: [] });
  }
  if (path === `${API}/rules`) {
    return json({ rules: [] });
  }
  if (path === `${API}/labels`) {
    return json({ labels: [] });
  }
  if (path === `${API}/assistant/profile`) {
    return json({ proactivity: "ask_first", asked_at: "2026-10-01T09:00:00Z", revision: 1 });
  }
  if (path === `${API}/rule-notices`) {
    return json({ notices: [] });
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function nudge({ page }: ScenarioContext): Promise<void> {
  Object.assign(state, { ana: "nudge", frames: [], cursor: 1 });
  const phone = isPhone(page);
  await page.route("**/bff/**", answer);

  await page.goto(`/w/${WORKSPACE}`);
  const row = page.getByText("Ana Costa hasn’t written yet");
  await row.waitFor();
  await caption(page, "João asked Ana, copying you, to send you the floor plan", 2400);
  await caption(page, "Ana still hasn't written, so Scout puts it under Nudge", 2400);

  // Ana's item: Nudge on 1 (the button on a phone).
  await row.click();
  await caption(page, "Nudge, Heard from them, Snooze, or Not waiting", 2000);
  await (phone
    ? page
        .getByRole("button", { name: /^Nudge Ana/u })
        .first()
        .click()
    : page.keyboard.press("1"));

  const dialog = page.getByRole("dialog", { name: "Nudge Ana" });
  await dialog.waitFor();
  await caption(page, "Scout writes a few lines to Ana, in the same thread, copying João", 2400);
  const text = dialog.getByRole("textbox", { name: "Your nudge" });
  await text.waitFor();
  await pause(1600);
  await dialog.getByRole("button", { name: /Shorter/u }).click();
  await caption(page, "Yours to edit, or make shorter. Nothing is sent before Send", 2200);
  await pause(1200);

  await (phone
    ? dialog.getByRole("button", { name: /^Send/u }).click()
    : text.press("ControlOrMeta+Enter"));
  await caption(page, "Sent, with Undo until it leaves. The item folds away", 2400);
  await pause(1200);

  // The nudge left: Scout waits on Ana again and checks on Thursday.
  state.ana = "waiting";
  changed();
  const waiting = page.getByRole("listitem", { name: "Waiting on Ana Costa" });
  await waiting.waitFor({ timeout: 20_000 });
  await waiting.scrollIntoViewIfNeeded();
  await caption(page, "Scout waits on Ana again and says when it checks", 2400);

  // Ana writes back, in a thread of her own: the wait ends.
  state.ana = "heard";
  changed();
  const heard = page.getByText("Heard back · Revised floor plan");
  await heard.waitFor({ timeout: 20_000 });
  await heard.scrollIntoViewIfNeeded();
  await caption(page, "Ana wrote, so Scout stopped waiting: it is in Done today", 2600);
}
