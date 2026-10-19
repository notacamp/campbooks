import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Pinned and Drafts beside Sent, Archive, Spam and Trash (campbooks-cloud#277),
// a Drafts row opening exactly its draft, and the phone chip row keeping the
// place you are in in view (#279). Needs only the Vite dev server: the
// browser's /bff calls are answered here with a fictional workspace, and the
// server's part is played by this file (the real one is the conversations
// module's tests).
//
//   pnpm --dir clients/app exec vite --port 5277
//   pnpm --dir clients record starred-drafts --base-url http://localhost:5277
//
// Fictional people on `example` domains only; nothing is sent.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const SAM_ID = "0192f1a0-0000-7000-8000-0000000000a1";
const API = `/bff/api/v1/w/${WORKSPACE}`;
const SAM = { name: "Sam Rivera", address: "sam@example.com" };
const MARTA = { name: "Marta Ferreira", address: "marta@example.com" };
const JOANA = { name: "Joana Reis", address: "joana@atelier-norte.example" };

const FIGURES = "0192f1a0-0000-7000-8000-0000000000c1";
const LEASE = "0192f1a0-0000-7000-8000-0000000000c2";
const PHOTOS = "0192f1a0-0000-7000-8000-0000000000c3";
const FIRST = "0192f1a0-0000-7000-8000-0000000000e1";
const SUBJECT = "Figures for the board";

function summary(id: string, subject: string, from: typeof MARTA, snippet: string, at: string) {
  return {
    id,
    inbox_id: INBOX,
    subject,
    stream: "primary",
    stream_source: "default",
    last_message_at: at,
    message_count: 1,
    unread_count: 0,
    starred: true,
    archived: false,
    trashed: false,
    spam: false,
    last_sent_at: null,
    sent_to: [],
    replied_since_sent: false,
    labels: [],
    suggestion: null,
    latest_from_you: false,
    latest_from: from,
    snippet,
  };
}

const PINNED = [
  summary(
    FIGURES,
    SUBJECT,
    MARTA,
    "The figures for the board are in the folder. Could you check two lines?",
    "2026-10-13T09:00:00Z",
  ),
  summary(
    LEASE,
    "Studio lease renewal",
    JOANA,
    "The landlord sent the new terms; the rent stays the same for a year.",
    "2026-10-09T16:30:00Z",
  ),
  {
    ...summary(
      PHOTOS,
      "Photos from Saturday",
      MARTA,
      "Here are the ones from the boat.",
      "2026-10-05T19:45:00Z",
    ),
    archived: true,
  },
];

const DETAIL = {
  conversation: PINNED[0],
  messages: [
    {
      id: FIRST,
      direction: "inbound",
      from: MARTA,
      to: [SAM],
      cc: [],
      reply_to: [],
      subject: SUBJECT,
      sent_at: "2026-10-13T09:00:00Z",
      read: true,
      starred: false,
      text: {
        content:
          "Hi Sam, the figures for the board are in the folder. Could you check the two lines in yellow before Thursday?",
        truncated: false,
      },
      has_html: false,
      attachments: [],
    },
  ],
  decisions: [],
};

function draft(
  id: string,
  kind: "new" | "reply" | "forward",
  to: (typeof MARTA)[],
  subject: string,
  body: string,
  at: string,
) {
  const thread = kind !== "new";
  return {
    id,
    inbox_id: INBOX,
    kind,
    conversation_id: thread ? FIGURES : null,
    message_id: thread ? FIRST : null,
    from: { name: SAM.name, address: SAM.address },
    to,
    cc: [],
    bcc: [],
    subject,
    body,
    quoted: thread
      ? "On 13 Oct 2026, 09:00, Marta Ferreira wrote:\n> The figures are in the folder."
      : "",
    attachments: [],
    version: 1,
    state: "draft",
    send_at: null,
    failure_reason: null,
    created_at: at,
    updated_at: at,
    content_token: `demo-token-${id.slice(-3)}`,
  };
}

const NEW_ID = "0192f1a0-0000-7000-8000-000000000101";
const REPLY_ID = "0192f1a0-0000-7000-8000-000000000102";
const FORWARD_ID = "0192f1a0-0000-7000-8000-000000000103";

const ALL = [
  draft(
    NEW_ID,
    "new",
    [JOANA],
    "Studio keys for next week",
    "Leaving the keys with the front desk on Monday.",
    "2026-10-13T14:30:00Z",
  ),
  draft(
    REPLY_ID,
    "reply",
    [MARTA],
    `Re: ${SUBJECT}`,
    "Thanks, Marta. I will check the two lines on Wednesday.",
    "2026-10-13T13:00:00Z",
  ),
  draft(
    FORWARD_ID,
    "forward",
    [JOANA],
    `Fwd: ${SUBJECT}`,
    "Joana, forwarding the board figures so you have them too.",
    "2026-10-12T16:00:00Z",
  ),
];

const state = { drafts: ALL.slice() };

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
      user: { sub: "demo-account", name: "Sam Rivera", email: SAM.address },
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
    return route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: `retry: 1000\n\nevent: ready\ndata: {"cursor":"1"}\n\n`,
    });
  }
  if (path === `${API}/members`) {
    return json({
      members: [
        {
          user_id: SAM_ID,
          display_name: "Sam Rivera",
          email: SAM.address,
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
          address: SAM.address,
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
        unread: stream === "primary" ? 2 : 0,
      })),
    });
  }
  if (path === `${API}/conversations`) {
    const view = url.searchParams.get("view");
    const stream = url.searchParams.get("stream");
    const list = view === "starred" ? PINNED : stream === "primary" ? [PINNED[0]] : [];
    return json({ conversations: list, next_cursor: null });
  }
  if (path === `${API}/conversations/${FIGURES}`) {
    return json(DETAIL);
  }
  if (path === `${API}/conversations/${FIGURES}/read`) {
    return json(PINNED[0]);
  }
  if (path === `${API}/conversations/${FIGURES}/rule-suggestions`) {
    return json({ shared: false, default: 0, none: null, suggestions: [] });
  }
  if (path === `${API}/drafts` && method === "GET") {
    const only = url.searchParams.get("conversation_id");
    return json({
      drafts: state.drafts.filter((d) => only === null || d.conversation_id === only),
    });
  }
  if (path === `${API}/drafts` && method === "POST") {
    // Undo: the same draft written again.
    const sent = (request.postDataJSON() ?? {}) as { subject?: string };
    const back =
      state.drafts.length < ALL.length ? ALL.find((d) => d.subject === sent.subject) : undefined;
    if (back !== undefined) {
      state.drafts = ALL.filter((d) => state.drafts.includes(d) || d === back);
    }
    return json(back ?? ALL[0], 201);
  }
  const one = /\/drafts\/([^/]+)$/u.exec(path);
  if (one !== null) {
    const found = state.drafts.find((d) => d.id === one[1]) ?? ALL.find((d) => d.id === one[1]);
    if (method === "DELETE") {
      state.drafts = state.drafts.filter((d) => d.id !== one[1]);
      return route.fulfill({ status: 204 });
    }
    return found === undefined ? json({ title: "Not found", status: 404 }, 404) : json(found);
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
  if (path === `${API}/attention`) {
    return json({ items: [], counts: [] });
  }
  if (path === `${API}/rule-notices`) {
    return json({ notices: [] });
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** G then `digit`, and wait for the place's heading. */
async function go(page: Page, digit: string, heading: string): Promise<void> {
  await page.keyboard.press("g");
  await page.keyboard.press(digit);
  await page.getByRole("heading", { level: 1, name: heading }).waitFor();
}

export default async function starredDrafts({ page }: ScenarioContext): Promise<void> {
  state.drafts = ALL.slice();
  await page.route("**/bff/**", answer);
  const phone = isPhone(page);

  await page.goto(`/w/${WORKSPACE}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();

  // 1. G then 9: Pinned.
  await go(page, "9", "Pinned");
  await page.getByText("Studio lease renewal").first().waitFor();
  await caption(
    page,
    phone
      ? "Pinned: what you pinned, archived or not, newest first"
      : "Pinned (G then 9): what you pinned, archived or not, newest first",
    2800,
  );

  // 2. G then 0: Drafts.
  await go(page, "0", "Drafts");
  await page.getByText("Studio keys for next week").first().waitFor();
  await caption(page, "Drafts (G then 0): a new message, a reply and a forward", 3000);

  // 3. The forward's row opens the forward, not the reply.
  await page.getByRole("button", { name: /^Draft · Joana Reis.*Fwd:/u }).click();
  const composer = page.getByRole("textbox", { name: "Message" });
  await composer.waitFor();
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if ((await composer.inputValue()).includes("forwarding the board figures")) {
      break;
    }
    await pause(200);
  }
  await caption(page, "The forward's row opens that draft, beside the Drafts list", 3000);
  await page.keyboard.press("Escape");
  await composer.waitFor({ state: "detached" }).catch(() => undefined);
  await page.keyboard.press("Escape");
  await page.getByRole("heading", { level: 1, name: "Drafts" }).waitFor();
  await caption(page, "Escape goes back to Drafts. Nothing was sent", 2200);

  // 4. Delete a draft, then Undo.
  await page.getByText("Studio keys for next week").first().waitFor();
  await page.getByRole("button", { name: "Delete draft: Studio keys for next week" }).click();
  await page.getByText("Draft deleted").waitFor();
  await caption(page, "Delete draft, with Undo in the toast", 2200);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await page.getByText("Studio keys for next week").first().waitFor();
  await caption(page, "Undo brings it back", 2000);

  // 5. The chip row keeps the place you are in in view.
  if (phone) {
    const chips = page.getByRole("group", { name: "Streams" });
    await caption(page, "On a phone, the places after the streams scroll sideways", 2200);
    await chips.getByRole("button", { name: "Trash" }).click();
    await page.getByRole("heading", { level: 1, name: "Trash" }).waitFor();
    await pause(900);
    await caption(page, "Trash is the last chip: the row scrolls so it stays in view", 3000);
    await page
      .getByRole("group", { name: "Streams" })
      .getByRole("button", { name: "Pinned" })
      .click();
    await page.getByRole("heading", { level: 1, name: "Pinned" }).waitFor();
    await pause(900);
    await caption(page, "And back to Pinned: its chip scrolls into view too", 3000);
  } else {
    await go(page, "8", "Trash");
    await caption(page, "G then 8 is still Trash: the older keys did not move", 2400);
  }
  await pause(800);
}
