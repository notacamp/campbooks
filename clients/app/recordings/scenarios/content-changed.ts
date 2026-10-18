import { createHash } from "node:crypto";

import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Send and Keep sign only what you were shown (campbooks-cloud#242). Needs
// only the Vite dev server: the browser's /bff calls are answered here with
// a fictional workspace, and the server's checks are played by this file
// (the real ones are the conversations module's tests).
//
//   pnpm --dir clients/app exec vite --port 5242
//   pnpm --dir clients record content-changed --base-url http://localhost:5242
//
// Sam opens a draft to Ana. Meanwhile the stored draft gains a Bcc it never
// showed him (what SQL in the workspace could do). Send is refused with 409
// `content_changed`: the composer shows the draft as it is now, Bcc
// included, and sends nothing. Sam removes the Bcc and sends. Then a rule
// waiting for his yes changes before he keeps it: Keep is refused, and the
// row reads what the rule does now. Fictional people and mail only.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const DRAFT = "0192f1a0-0000-7000-8000-0000000000d1";
const RULE = "0192f1a0-0000-7000-8000-0000000000f1";
const SAM = "0192f1a0-0000-7000-8000-0000000000a1";
const API = `/bff/api/v1/w/${WORKSPACE}`;

interface Address {
  name: string | null;
  address: string;
}

/** The draft as the "server" stores it. */
const draft = {
  version: 1,
  to: [{ name: "Ana Lopes", address: "ana@example.com" }] as Address[],
  cc: [] as Address[],
  bcc: [] as Address[],
  subject: "Workshop on 20 October",
  body: "Hi Ana,\n\nCould you book two seats for the workshop?\n\nSam",
  state: "draft",
  sendAt: null as string | null,
};

/** The rule waiting for a yes, as stored. */
const rule = { trash: false };

/** A stand-in for the server's token: it changes whenever what is stored does. */
function tokenOf(content: unknown): string {
  return `demo-${createHash("sha256").update(JSON.stringify(content)).digest("hex").slice(0, 16)}`;
}

function draftView() {
  const { version, to, cc, bcc, subject, body } = draft;
  return {
    id: DRAFT,
    inbox_id: INBOX,
    kind: "new",
    conversation_id: null,
    message_id: null,
    from: { name: "Sam Rivera", address: "sam@atelier-norte.example" },
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    body: draft.body,
    quoted: "",
    attachments: [],
    version: draft.version,
    state: draft.state,
    send_at: draft.sendAt,
    failure_reason: null,
    created_at: "2026-10-18T09:00:00Z",
    updated_at: "2026-10-18T09:05:00Z",
    content_token: tokenOf({ version, to, cc, bcc, subject, body }),
  };
}

function ruleView() {
  const actions = rule.trash
    ? [{ action: "trash" }]
    : [{ action: "move_to_stream", stream: "newsletters" }];
  const conditions = [{ field: "sender_address", value: "news@daily-brief.example" }];
  return {
    id: RULE,
    inbox_id: INBOX,
    teaching_id: "0192f1a0-0000-7000-8000-0000000000e1",
    conditions,
    actions,
    origin: "correction",
    origin_text: null,
    state: "proposed",
    taught_by: SAM,
    from_conversation_id: null,
    created_at: "2026-10-16T09:00:00Z",
    confirmed_at: null,
    applied_count: 0,
    last_applied_at: null,
    recent_exceptions: 0,
    run: null,
    shared: false,
    may_change: true,
    needs_confirmation: false,
    content_token: tokenOf({ conditions, actions }),
  };
}

async function answer(route: Route): Promise<void> {
  const request = route.request();
  const path = new URL(request.url()).pathname;
  const method = request.method();
  const json = (body: unknown, status = 200) =>
    route.fulfill({
      status,
      contentType: status >= 400 ? "application/problem+json" : "application/json",
      body: JSON.stringify(body),
    });
  const changed = () =>
    json(
      {
        type: "about:blank",
        title: "Conflict",
        status: 409,
        detail: "This changed since you opened it. Check it and try again.",
        code: "content_changed",
      },
      409,
    );
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
        conversations: 0,
        unread: 0,
      })),
    });
  }
  if (path === `${API}/drafts/${DRAFT}` && method === "GET") {
    return json(draftView());
  }
  if (path === `${API}/drafts/${DRAFT}` && method === "PATCH") {
    if (sent["version"] !== draft.version) {
      return json(
        { type: "about:blank", title: "Conflict", status: 409, code: "draft_changed" },
        409,
      );
    }
    for (const field of ["to", "cc", "bcc", "subject", "body"] as const) {
      if (sent[field] !== undefined) {
        Object.assign(draft, { [field]: sent[field] });
      }
    }
    draft.version += 1;
    return json(draftView());
  }
  if (path === `${API}/drafts/${DRAFT}/send` && method === "POST") {
    // What the server does: sign only the draft the token was given with.
    if (sent["version"] !== draft.version || sent["content_token"] !== draftView().content_token) {
      return changed();
    }
    draft.state = "scheduled";
    draft.sendAt = new Date(Date.now() + 10_000).toISOString();
    return json(draftView(), 202);
  }
  if (path === `${API}/drafts`) {
    return json({ drafts: [] });
  }
  if (path === `${API}/conversations`) {
    return json({ conversations: [], next_cursor: null });
  }
  if (path === `${API}/rules`) {
    return json({ rules: [ruleView()] });
  }
  if (path === `${API}/rules/${RULE}/confirm` && method === "POST") {
    if (sent["content_token"] !== ruleView().content_token) {
      return changed();
    }
    return json({ ...ruleView(), state: "active" });
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
  if (path.endsWith("/changes")) {
    return route.abort();
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function contentChanged({ page }: ScenarioContext): Promise<void> {
  Object.assign(draft, {
    version: 1,
    cc: [],
    bcc: [],
    state: "draft",
    sendAt: null,
  });
  rule.trash = false;
  const phone = isPhone(page);
  await page.route("**/bff/**", answer);

  // A draft, as Sam's composer opened it.
  await page.goto(`/w/${WORKSPACE}/compose?draft=${DRAFT}`);
  const composer = page.getByRole("region", { name: "New message" });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "Sam's draft to Ana: no Cc, no Bcc", 2200);

  // What SQL in the workspace could do: change the stored draft, version kept.
  draft.bcc = [{ name: null, address: "eve@example.com" }];
  await caption(page, "Meanwhile the stored draft gains a Bcc, its version unchanged", 2600);

  await caption(page, phone ? "Send" : "⌘↵ sends", 1200);
  await (phone
    ? composer.getByRole("button", { name: /^Send/u }).click()
    : body.press("ControlOrMeta+Enter"));
  await composer.getByText("This changed since you opened it. Check it and send again.").waitFor();
  await composer.getByText("eve@example.com").waitFor();
  await caption(page, "Refused: nothing is sent. The draft shows as it is now, Bcc included", 3200);

  await composer.getByRole("button", { name: "Remove eve@example.com" }).click();
  await composer.getByText("Saved").waitFor();
  await caption(page, "Sam takes the Bcc out", 1800);
  await (phone
    ? composer.getByRole("button", { name: /^Send/u }).click()
    : body.press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to Ana Lopes/u).waitFor();
  await caption(page, "Sent as Sam saw it, with the usual Undo", 2400);

  // A rule waiting for Sam's yes.
  await page.goto(`/w/${WORKSPACE}/settings/memory`);
  const waiting = page.getByRole("list", { name: "Waiting for you" });
  await waiting.waitFor();
  await waiting.scrollIntoViewIfNeeded();
  await caption(page, "Scout suggests a rule: mail from the daily brief goes to Newsletters", 2600);
  rule.trash = true;
  await caption(
    page,
    "Meanwhile the stored suggestion is changed to put that mail in the trash",
    2800,
  );
  await waiting.getByRole("button", { name: "Keep this rule" }).click();
  await page
    .getByText("This rule changed since you opened it. Check it and keep it again.")
    .waitFor();
  await waiting.getByText(/put in the trash/u).waitFor();
  await caption(page, "Not kept: the row reads what it does now, still waiting for a yes", 3200);
  await pause(800);
}
