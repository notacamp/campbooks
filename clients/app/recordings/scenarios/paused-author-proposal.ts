import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Keeping a suggested rule whose author is away (campbooks-cloud#197).
// Needs only the Vite dev server: the browser's /bff calls are answered here
// with fictional people and rules, so nothing else has to run.
//
//   pnpm --dir clients/app exec vite --port 5197
//   pnpm --dir clients record paused-author-proposal --base-url http://localhost:5197
//
// Scout's memory lists two suggestions waiting for a yes. Leo Marques taught
// one of them and is away for now: before Keep this rule, its row says it
// stays paused once kept, naming no cause. Sam keeps it anyway; the toast
// says it is kept and paused for now, and the rule moves under the inbox as
// paused. Fictional people and mail only.

const WORKSPACE = "0192f1a0-0000-7000-8000-000000000001";
const INBOX = "0192f1a0-0000-7000-8000-0000000000b1";
const API = `/bff/api/v1/w/${WORKSPACE}`;
const SAM = "0192f1a0-0000-7000-8000-0000000000a1";
const LEO = "0192f1a0-0000-7000-8000-0000000000a2";

const rule = (n: number, sender: string, author: string) => ({
  id: `0192f1a0-0000-7000-8000-0000000000f${String(n)}`,
  inbox_id: INBOX,
  teaching_id: `0192f1a0-0000-7000-8000-0000000000e${String(n)}`,
  conditions: [{ field: "sender_address", value: sender }],
  actions: [{ action: "move_to_stream", stream: "newsletters" }],
  origin: "correction",
  origin_text: null,
  state: "proposed",
  taught_by: author,
  from_conversation_id: null,
  created_at: "2026-10-16T09:00:00Z",
  confirmed_at: null,
  applied_count: 0,
  last_applied_at: null,
  recent_exceptions: 0,
  run: null,
  shared: true,
  may_change: true,
});

const LEOS = {
  ...rule(1, "digest@design-notes.example", LEO),
  paused_once_kept: "author_unavailable",
};
const SAMS = rule(2, "news@daily-brief.example", SAM);

let kept = false;

function rules() {
  const leos = kept
    ? {
        ...LEOS,
        state: "paused",
        paused_reason: "author_unavailable",
        paused_once_kept: undefined,
        confirmed_at: "2026-10-17T09:00:00Z",
      }
    : LEOS;
  return { rules: [leos, SAMS] };
}

async function answer(route: Route): Promise<void> {
  const request = route.request();
  const path = new URL(request.url()).pathname;
  const json = (body: unknown, status = 200) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
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
        {
          user_id: LEO,
          display_name: "Leo Marques",
          email: "leo.marques@example.com",
          role: "member",
          joined_at: "2026-09-03T09:00:00Z",
          you: false,
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
          address: "studio@atelier-norte.example",
          display_name: "Atelier Norte",
          role: "manager",
          read_only: false,
          unread_count: 0,
        },
      ],
    });
  }
  if (path === `${API}/streams`) {
    return json({
      streams: [
        { stream: "primary", conversations: 0, unread: 0 },
        { stream: "updates", conversations: 0, unread: 0 },
        { stream: "receipts", conversations: 0, unread: 0 },
        { stream: "newsletters", conversations: 0, unread: 0 },
      ],
    });
  }
  if (path === `${API}/rules`) {
    return json(rules());
  }
  if (path === `${API}/rules/${LEOS.id}/confirm` && request.method() === "POST") {
    kept = true;
    return json(rules().rules[0]);
  }
  if (path === `${API}/assistant/profile`) {
    return json({ proactivity: "ask_first", asked_at: "2026-10-01T09:00:00Z", revision: 1 });
  }
  if (path === `${API}/attention`) {
    return json({ items: [], counts: [] });
  }
  if (path.endsWith("/changes")) {
    return route.abort();
  }
  return json({ type: "about:blank", title: "Not in this recording", status: 404 }, 404);
}

async function scrollTo(page: Page, text: RegExp): Promise<void> {
  await page.getByText(text).first().scrollIntoViewIfNeeded();
  await pause(500);
}

export default async function pausedAuthorProposal({ page }: ScenarioContext): Promise<void> {
  kept = false;
  await page.route("**/bff/**", answer);
  await page.goto(`/w/${WORKSPACE}/settings/memory`);
  await page.getByRole("list", { name: "Waiting for you" }).waitFor();
  await scrollTo(page, /Waiting for you/u);
  await caption(page, "Scout's memory: two suggested rules wait for a yes", 2200);

  const waiting = page.getByRole("list", { name: "Waiting for you" });
  await waiting.getByText(/^If kept, it stays paused/u).waitFor();
  await scrollTo(page, /^If kept, it stays paused/u);
  await caption(
    page,
    "Leo taught this one and is away for now: it says so before you keep it",
    3000,
  );
  await caption(page, "It never says why Leo is away. The other suggestion has no note", 2600);

  await waiting.getByRole("button", { name: "Keep this rule" }).first().click();
  await page.getByText(/^Rule kept, paused for now/u).waitFor();
  await caption(page, "Kept: the toast says it is paused for now", 2600);
  await scrollTo(page, /^Paused for now; it acts again on its own/u);
  await caption(page, "It stays Leo's, paused until he is back or someone takes it over", 2800);
}
