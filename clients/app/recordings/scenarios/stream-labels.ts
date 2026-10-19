import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { api, caption, signedIn } from "./inbox.ts";

// Streams reach Gmail both ways (campbooks-cloud#292), on the real stack with
// the fake Google, as the `inbox` scenario runs it (see its header for the
// commands). A move to Receipts lands in Gmail's Updates tab with the
// `Campbooks/Receipts` label, as Gmail has no Receipts tab; dragging the
// message to Promotions in Gmail (`POST /fake/relabel`, as the member would)
// moves the conversation to Newsletters, and Campbooks takes the stale label
// off. The fake's `GET /fake/mailbox` shows what the demo Gmail holds.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record stream-labels \
//     --base-url http://localhost:5181
//
// Fictional person and mail only; no token is ever shown.

const fakeGoogle = process.env["FAKE_GOOGLE_URL"] ?? "http://127.0.0.1:9071";
const subject = "Your invoice CH-20418 for €148.00";
const MARK = "Campbooks/Receipts";

/** The labels the fake Gmail holds on the message, user labels by name. */
async function gmailLabels(page: Page): Promise<string[]> {
  const response = await page.request.get(`${fakeGoogle}/fake/mailbox`);
  const { mailboxes } = (await response.json()) as {
    mailboxes: { subject: string; labels: string[] }[][];
  };
  // The newest account is this recording's (each run signs up afresh).
  return mailboxes.at(-1)?.find((message) => message.subject === subject)?.labels ?? [];
}

/** Poll the fake Gmail until `done` holds of the message's labels (30 s at most). */
async function gmailUntil(page: Page, done: (labels: string[]) => boolean): Promise<string[]> {
  let labels = await gmailLabels(page);
  for (let attempt = 0; attempt < 60 && !done(labels); attempt += 1) {
    await pause(500);
    labels = await gmailLabels(page);
  }
  return labels;
}

/** A panel showing what Gmail holds, so the video shows the provider side. */
async function showGmail(page: Page, labels: string[]): Promise<void> {
  const text = `Gmail (fake Google): the invoice has ${labels.join(", ")}`;
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-gmail") ?? document.createElement("div");
    box.id = "recording-gmail";
    box.textContent = ${JSON.stringify(text)};
    const phone = window.innerWidth < 768;
    Object.assign(box.style, {
      position: "fixed", zIndex: "2147483647", pointerEvents: "none",
      left: "8px", right: phone ? "8px" : "auto", bottom: phone ? "8px" : "16px",
      maxWidth: phone ? "none" : "560px", padding: phone ? "6px 10px" : "10px 14px",
      borderRadius: "10px", background: "rgba(26, 115, 232, 0.92)", color: "#fff",
      font: phone ? "500 12px/1.35 system-ui, sans-serif" : "500 15px/1.4 system-ui, sans-serif",
    });
    document.body.append(box);
  })()`);
}

interface Listed {
  id: string;
  subject: string;
  stream: string;
}

/** The invoice's conversation, as the API lists it. */
async function invoice(page: Page, workspace: string): Promise<Listed | undefined> {
  const { conversations } = (await api(
    page,
    "GET",
    `/bff/api/v1/w/${workspace}/conversations?limit=100`,
  )) as { conversations: Listed[] };
  return conversations.find((conversation) => conversation.subject === subject);
}

/** Wait until the invoice's conversation is in `stream` (30 s at most). */
async function streamUntil(page: Page, workspace: string, stream: string): Promise<void> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    if ((await invoice(page, workspace))?.stream === stream) {
      return;
    }
    await pause(500);
  }
}

/** Run the mailbox's sync now, as "Sync now" does. */
async function syncNow(page: Page, workspace: string): Promise<void> {
  const { connections } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/connections`)) as {
    connections: { id: string }[];
  };
  await api(
    page,
    "POST",
    `/bff/api/v1/w/${workspace}/connections/${connections[0]?.id ?? ""}/sync`,
  );
}

export default async function streamLabels({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  // a. The invoice, in Primary or Updates, and what Gmail holds. (The page
  // opens first: the API is called from it, through the BFF.)
  await page.goto(`/w/${workspace}/inbox/primary`);
  const start = (await invoice(page, workspace))?.stream ?? "primary";
  if (start !== "primary") {
    await page.goto(`/w/${workspace}/inbox/${start}`);
  }
  await page.getByText(subject).first().click();
  await page.getByRole("heading", { name: subject }).waitFor();
  await showGmail(page, await gmailLabels(page));
  await caption(page, "An invoice, in Gmail's Updates tab", 2400);

  // b. Move it to Receipts.
  // The keys at both widths: at 375 the Move menu opens off-screen from an
  // open conversation (campbooks-cloud#301).
  await caption(page, "M, then 3: move it to Receipts", 1400);
  await page.keyboard.press("m");
  await pause(700);
  await page.keyboard.press("3");
  // The connector writes it back on the stream's next run.
  const written = await gmailUntil(page, (labels) => labels.includes(MARK));
  await showGmail(page, written);
  await caption(page, "Gmail has no Receipts tab, so Campbooks labels it", 3000);

  // c. The member drags it to Promotions in Gmail; the mark stays on.
  await page.request.post(`${fakeGoogle}/fake/relabel`, {
    data: { subject, add: ["CATEGORY_PROMOTIONS"], remove: ["CATEGORY_UPDATES"] },
  });
  await showGmail(page, await gmailLabels(page));
  await caption(page, "In Gmail, dragged to the Promotions tab", 2600);
  await syncNow(page, workspace);
  await streamUntil(page, workspace, "newsletters");
  await page.goto(`/w/${workspace}/inbox/newsletters`);
  await page.getByRole("heading", { level: 1, name: "Newsletters" }).waitFor();
  await page.getByText(subject).first().waitFor();
  await caption(page, "Campbooks follows: it is in Newsletters now", 2600);
  // The stale Receipts label comes off on the next push.
  const tidied = await gmailUntil(page, (labels) => !labels.includes(MARK));
  await showGmail(page, tidied);
  await caption(page, "…and the Receipts label comes off in Gmail", 3000);
}
