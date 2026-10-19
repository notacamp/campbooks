import { createHash } from "node:crypto";

import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Send recovers when the draft was saved elsewhere (campbooks-cloud#242).
// Needs only the Vite dev server: the browser's /bff calls are answered here
// with a fictional workspace, and the server's checks are played by this
// file (the real ones are the conversations module's tests).
//
//   pnpm --dir clients/app exec vite --port 5242
//   pnpm --dir clients record draft-changed --base-url http://localhost:5242
//
// Sam has a draft to Ana open, nothing unsaved. Meanwhile he adds a line
// to the same draft on his phone: the stored draft is at a newer version.
// Send is refused with 409 `draft_changed`, as the server does: the
// composer reads the draft again, shows the line from the phone with the
// notice, and sends nothing. Sam checks it and presses Send again: it goes,
// with the usual Undo. Before, every Send was refused the same way until
// the composer was closed and opened again. Fictional people and mail only.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const DRAFT = "0192f1a0-0000-7000-8000-0000000000d2";
const SAM = "0192f1a0-0000-7000-8000-0000000000a1";
const API = `/bff/api/v1/w/${WORKSPACE}`;

const OPENED = "Hi Ana,\n\nCould you book two seats for the workshop?\n\nSam";
const FROM_PHONE =
  "Hi Ana,\n\nCould you book two seats for the workshop?\n\nPS: vegetarian lunch for both of us, please.\n\nSam";

/** The draft as the "server" stores it. */
const draft = {
  version: 1,
  body: OPENED,
  state: "draft",
  sendAt: null as string | null,
};

const TO = [{ name: "Ana Lopes", address: "ana@example.com" }];
const SUBJECT = "Workshop on 20 October";

/** A stand-in for the server's token: it changes whenever what is stored does. */
function tokenOf(content: unknown): string {
  return `demo-${createHash("sha256").update(JSON.stringify(content)).digest("hex").slice(0, 16)}`;
}

function draftView() {
  const { version, body } = draft;
  return {
    id: DRAFT,
    inbox_id: INBOX,
    kind: "new",
    conversation_id: null,
    message_id: null,
    from: { name: "Sam Rivera", address: "sam@atelier-norte.example" },
    to: TO,
    cc: [],
    bcc: [],
    subject: SUBJECT,
    body,
    quoted: "",
    attachments: [],
    version,
    state: draft.state,
    send_at: draft.sendAt,
    failure_reason: null,
    created_at: "2026-10-18T09:00:00Z",
    updated_at: "2026-10-18T09:05:00Z",
    content_token: tokenOf({ version, to: TO, subject: SUBJECT, body }),
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
  const conflict = (code: string) =>
    json({ type: "about:blank", title: "Conflict", status: 409, code }, 409);
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
      return conflict("draft_changed");
    }
    if (sent["content_token"] !== draftView().content_token) {
      return conflict("content_changed");
    }
    if (typeof sent["body"] === "string") {
      draft.body = sent["body"];
    }
    draft.version += 1;
    return json(draftView());
  }
  if (path === `${API}/drafts/${DRAFT}/send` && method === "POST") {
    // What the server does: the saved version first, then the token.
    if (sent["version"] !== draft.version) {
      return conflict("draft_changed");
    }
    if (sent["content_token"] !== draftView().content_token) {
      return conflict("content_changed");
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

/** Wait for `check` to hold (what a locator cannot wait for: a field's value). */
async function until(check: () => Promise<boolean>): Promise<void> {
  for (let tries = 0; tries < 50; tries += 1) {
    if (await check()) {
      return;
    }
    await pause(100);
  }
  throw new Error("timed out waiting for the draft as it is now");
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function draftChanged({ page }: ScenarioContext): Promise<void> {
  Object.assign(draft, { version: 1, body: OPENED, state: "draft", sendAt: null });
  const phone = isPhone(page);
  await page.route("**/bff/**", answer);

  // A draft, as Sam's composer opened it, with nothing unsaved.
  await page.goto(`/w/${WORKSPACE}/compose?draft=${DRAFT}`);
  const composer = page.getByRole("region", { name: "New message" });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "Sam's draft to Ana, open here with nothing unsaved", 2200);

  // Saved elsewhere: Sam adds a line on his phone.
  draft.body = FROM_PHONE;
  draft.version += 1;
  await caption(page, "Meanwhile Sam adds a line to it on his phone", 2600);

  const send = async () => {
    await (phone
      ? composer.getByRole("button", { name: /^Send/u }).click()
      : body.press("ControlOrMeta+Enter"));
  };
  await caption(page, phone ? "Send" : "⌘↵ sends", 1200);
  await send();
  await composer.getByText("This changed since you opened it. Check it and send again.").waitFor();
  await until(async () => (await body.inputValue()).includes("vegetarian lunch"));
  await caption(page, "Refused, nothing sent: it shows as it is now, with the new line", 3200);

  await caption(page, "Sam checks it and sends again", 1600);
  await send();
  await page.getByText(/^Sending to Ana Lopes/u).waitFor();
  await caption(page, "Sent as Sam saw it, with the usual Undo. No need to reopen it", 2600);
  await pause(800);
}
