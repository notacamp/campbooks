import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// One "Search or ask" menu for conversations and commands (campbooks-cloud#154).
// Needs only the Vite dev server: the browser's /bff calls are answered here
// with a few fictional conversations, so nothing else has to run.
//
//   pnpm --dir clients/app exec vite --port 5183
//   pnpm --dir clients record unified-search --base-url http://localhost:5183
//
// Desktop: ⌘K opens the menu, typing lists conversations first (matched words
// marked), then commands, then Ask; ↓ moves across all of them, ↵ opens a
// hit in the inbox reader; `/` opens the very same menu; a filter chip;
// ⌘↵ opens all results. Phone: `/` and the top bar's button still open the
// results page. Fictional mail only.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const API = `/bff/api/v1/w/${WORKSPACE}`;

const sender = (name: string, address: string) => ({ name, address });
const summary = (
  n: number,
  subject: string,
  from: { name: string; address: string },
  stream: string,
  snippet: string,
  unread: number,
) => ({
  id: `0192f1a0-0000-7000-8000-0000000000c${String(n)}`,
  inbox_id: INBOX,
  subject,
  stream,
  stream_source: "default",
  archived: false,
  labels: [],
  suggestion: null,
  last_message_at: `2026-10-1${String(n)}T09:00:00Z`,
  message_count: 1,
  unread_count: unread,
  starred: false,
  latest_from: from,
  snippet,
});

const CONVERSATIONS = [
  summary(
    1,
    "Your invoice CH-20418 for €148.00",
    sender("Cloudhost Billing", "billing@cloudhost.example"),
    "receipts",
    "Your invoice is due on 16 October.",
    0,
  ),
  summary(
    2,
    "Lease for the studio: signed copy",
    sender("Joana Reis", "joana@atelier-norte.example"),
    "primary",
    "Here is the signed lease for the studio, with the invoice for the deposit.",
    1,
  ),
  summary(
    3,
    "Q4 plan: can you review by Friday?",
    sender("Maya Okafor", "maya@northwind-studio.example"),
    "primary",
    "The invoice schedule is unchanged.",
    1,
  ),
  summary(
    4,
    "New address for your inbox",
    sender("Atelier Norte", "hello@atelier-norte.example"),
    "updates",
    "From Monday, the shared inbox moves to a new address.",
    0,
  ),
];

/** The words of `text` that start with `needle`, as the API's matched segments. */
function mark(text: string, needle: string): { text: string; hit: boolean }[] {
  const at = text.toLowerCase().indexOf(needle.toLowerCase());
  if (needle === "" || at === -1) {
    return [{ text, hit: false }];
  }
  const end = at + needle.length;
  return [
    { text: text.slice(0, at), hit: false },
    { text: text.slice(at, end), hit: true },
    { text: text.slice(end), hit: false },
  ].filter((part) => part.text !== "");
}

function search(url: URL) {
  const q = (url.searchParams.get("q") ?? "").trim();
  const words = q.split(/\s+/u).filter((word) => !word.includes(":"));
  const text = words.join(" ");
  const filters = q
    .split(/\s+/u)
    .filter((word) => word.startsWith("from:"))
    .map((word) => ({ operator: "from", value: word.slice(5) }));
  const hits = CONVERSATIONS.filter((conversation) =>
    `${conversation.subject} ${conversation.snippet}`.toLowerCase().includes(text.toLowerCase()),
  )
    .filter((conversation) =>
      filters.every((filter) =>
        conversation.latest_from.address.toLowerCase().includes(filter.value.toLowerCase()),
      ),
    )
    .map((conversation) => ({
      record: { type: "conversations.conversation", id: conversation.id },
      container: { type: "conversations.inbox", id: INBOX },
      title: mark(conversation.subject, text),
      snippet: mark(conversation.snippet, text),
      from: conversation.latest_from,
      occurred_at: conversation.last_message_at,
      facets: {
        stream: conversation.stream,
        unread: conversation.unread_count > 0,
        starred: false,
        count: 1,
      },
    }));
  return { hits, indexing: false, next_cursor: null, query: { text, filters, warnings: [] } };
}

function detail(id: string) {
  const conversation = CONVERSATIONS.find((each) => each.id === id) ?? CONVERSATIONS[0];
  return {
    conversation,
    decisions: [],
    messages: [
      {
        id: "0192f1a0-0000-7000-8000-0000000000e1",
        direction: "inbound",
        from: conversation?.latest_from,
        to: [sender("Sam Rivera", "sam.rivera@example.com")],
        cc: [],
        reply_to: [],
        subject: conversation?.subject,
        sent_at: conversation?.last_message_at,
        read: false,
        starred: false,
        text: { content: conversation?.snippet, truncated: false },
        has_html: false,
        attachments: [],
      },
    ],
  };
}

async function answer(route: Route): Promise<void> {
  const url = new URL(route.request().url());
  const path = url.pathname;
  const json = (body: unknown, status = 200) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
  if (path === "/bff/session") {
    return json({
      authenticated: true,
      user: { sub: "demo-account", name: "Ana Duarte", email: "ana@example.com" },
      expires_at: "2099-01-01T00:00:00Z",
    });
  }
  if (path === "/bff/api/v1/workspaces") {
    return json({
      workspaces: [
        {
          id: WORKSPACE,
          name: "Duarte & Pessoa",
          kind: "business",
          accent: "business",
          role: "owner",
          created_at: "2026-09-01T09:00:00Z",
        },
      ],
    });
  }
  if (path === `${API}/search`) {
    return json(search(url));
  }
  if (path === `${API}/inboxes`) {
    return json({
      inboxes: [
        {
          id: INBOX,
          channel: "email",
          address: "sam.rivera@example.com",
          display_name: "Sam Rivera",
          role: "manager",
          read_only: false,
          unread_count: 2,
        },
      ],
    });
  }
  if (path === `${API}/streams`) {
    return json({
      streams: [
        { stream: "primary", conversations: 2, unread: 2 },
        { stream: "updates", conversations: 0, unread: 0 },
        { stream: "receipts", conversations: 1, unread: 0 },
        { stream: "newsletters", conversations: 0, unread: 0 },
      ],
    });
  }
  if (path === `${API}/conversations`) {
    const stream = url.searchParams.get("stream");
    return json({
      conversations: CONVERSATIONS.filter((each) => each.stream === stream),
      next_cursor: null,
    });
  }
  if (path.startsWith(`${API}/conversations/`)) {
    return json(detail(path.slice(path.lastIndexOf("/") + 1)));
  }
  if (path.endsWith("/changes")) {
    return route.abort();
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

async function typeSlowly(page: Page, text: string): Promise<void> {
  await page.keyboard.type(text, { delay: 90 });
  // The menu waits ~120 ms after the last key before asking.
  await pause(700);
}

export default async function unifiedSearch({ page }: ScenarioContext): Promise<void> {
  await page.route("**/bff/**", answer);
  await page.goto(`/w/${WORKSPACE}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();

  if (isPhone(page)) {
    await page.keyboard.press("Meta+k");
    await page.getByRole("combobox", { name: "Search, ask, or run a command" }).waitFor();
    await typeSlowly(page, "invoice");
    await page.getByRole("option", { name: /Cloudhost Billing/u }).waitFor();
    await caption(page, "With a keyboard attached, the menu fits a narrow screen", 2400);
    await page.keyboard.press("Escape");
    await pause(400);
    await caption(page, "On a phone, search is still a button in the top bar");
    await page.getByRole("button", { name: "Search conversations" }).click();
    await page.getByRole("searchbox", { name: "Search conversations" }).waitFor();
    await typeSlowly(page, "invoice");
    await page.getByRole("button", { name: /Cloudhost Billing/u }).waitFor();
    await caption(page, "…and opens the results page, as before", 2200);
    return;
  }

  await caption(page, "One field in the sidebar: Search or ask, with ⌘K and / beside it", 2200);
  await page.keyboard.press("Meta+k");
  const field = page.getByRole("combobox", { name: "Search, ask, or run a command" });
  await field.waitFor();
  await caption(page, "⌘K opens the menu", 1400);
  await typeSlowly(page, "invoice");
  await page.getByRole("option", { name: /Cloudhost Billing/u }).waitFor();
  await caption(page, "Conversations come first, with the matched words marked", 2400);
  await page.keyboard.press("Meta+a");
  await typeSlowly(page, "inbox");
  await page.getByRole("option", { name: /New address for your inbox/u }).waitFor();
  await page.keyboard.press("ArrowDown");
  await pause(500);
  await page.keyboard.press("ArrowDown");
  await caption(page, "One cursor: ↓ moves on from the conversations into the commands", 2800);
  await page.keyboard.press("Escape");
  await pause(500);

  await caption(page, "/ opens the very same menu", 1600);
  await page.keyboard.press("/");
  await field.waitFor();
  await typeSlowly(page, "lease from:joana");
  await page.getByRole("button", { name: "Remove filter: From joana" }).waitFor();
  await caption(page, "The filter it understood is a chip under the field", 2400);
  await page.keyboard.press("Enter");
  await page.getByRole("heading", { name: "Lease for the studio: signed copy" }).waitFor();
  await caption(page, "↵ opens the conversation in the inbox reader", 2400);

  await page.keyboard.press("/");
  await field.waitFor();
  await typeSlowly(page, "invoice");
  await page.getByRole("option", { name: /Cloudhost Billing/u }).waitFor();
  await caption(page, "⌘↵ (or the last row) shows all results", 1800);
  await page.keyboard.press("Meta+Enter");
  await page.getByRole("region", { name: "Search results" }).waitFor();
  await caption(page, "The results page, as before", 2200);
}
