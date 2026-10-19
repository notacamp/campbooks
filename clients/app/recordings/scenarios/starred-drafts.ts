import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// Pinned and Drafts beside Sent, Archive, Spam and Trash (campbooks-cloud#277),
// and the phone chip row keeping the folder you are in in view (#279), on the
// real stack: start it as inbox.ts says. Nothing is sent; the drafts are
// written to the demo mailbox's own inbox only.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record starred-drafts \
//     --base-url http://localhost:5181
//
// Fictional people on `example` domains only.

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** POST `body` as JSON through the BFF, as the app does. */
async function post(page: Page, path: string, body: unknown): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: "POST",
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
    body: ${JSON.stringify(JSON.stringify(body))},
  }).then((response) => response.status === 204 ? null : response.json())`);
}

/** G then `digit`, and wait for the place's heading. */
async function go(page: Page, digit: string, heading: string): Promise<void> {
  await page.keyboard.press("g");
  await page.keyboard.press(digit);
  await page.getByRole("heading", { level: 1, name: heading }).waitFor();
}

const FIGURES = "Figures for the board";
const KEYS = "Studio keys for next week";

export default async function starredDrafts({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const base = `/bff/api/v1/w/${workspace}`;
  await page.goto(`/w/${workspace}`);

  // Something to pin, and two drafts to find in Drafts.
  const { conversations } = (await api(
    page,
    "GET",
    `${base}/conversations?stream=primary&limit=2`,
  )) as { conversations: { id: string }[] };
  for (const conversation of conversations) {
    await post(page, `${base}/conversations/${conversation.id}/star`, {});
  }
  const { inboxes } = (await api(page, "GET", `${base}/inboxes`)) as { inboxes: { id: string }[] };
  const inbox = inboxes[0]?.id ?? "";
  await post(page, `${base}/drafts`, {
    kind: "new",
    inbox_id: inbox,
    to: [{ name: "Marta Ferreira", address: "marta@example.com" }],
    subject: FIGURES,
    body: "Marta, the figures are in the folder. Two lines still need your eye.",
  });
  await post(page, `${base}/drafts`, {
    kind: "new",
    inbox_id: inbox,
    to: [{ name: "Sam Rivera", address: "sam@example.com" }],
    subject: KEYS,
    body: "Leaving the keys with the front desk on Monday.",
  });

  // 1. G then 9: Pinned.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await go(page, "9", "Pinned");
  await caption(
    page,
    phone
      ? "Pinned: what you pinned, in the chip row after Sent"
      : "Pinned (G then 9): what you pinned, archived or not, newest first",
    2600,
  );

  // 2. G then 0: Drafts.
  await go(page, "0", "Drafts");
  await page.getByText(FIGURES).first().waitFor();
  await caption(page, "Drafts (G then 0): who each is to, the subject, a line of the words", 2800);

  // 3. Open a draft, then leave without sending.
  await page.getByText(FIGURES).first().click();
  const body = page.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "Opening a draft opens the composer where you left it", 2200);
  await body.press("Escape");
  await page.getByRole("heading", { level: 1 }).first().waitFor();
  await caption(page, "Closing it keeps the draft. Nothing was sent", 2000);

  // 4. Delete a draft, then Undo.
  await page.goto(`/w/${workspace}/inbox/drafts`);
  await page.getByText(KEYS).first().waitFor();
  await page.getByRole("button", { name: `Delete draft: ${KEYS}` }).click();
  await page.getByText("Draft deleted").waitFor();
  await caption(page, "Delete draft, with Undo in the toast", 2000);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await page.getByText(KEYS).first().waitFor();
  await caption(page, "Undo brings it back", 1800);

  // 5. On a phone, a tap on the last chip scrolls it into view.
  if (phone) {
    const chips = page.getByRole("group", { name: "Streams" });
    await chips.getByRole("button", { name: "Trash" }).click();
    await page.getByRole("heading", { level: 1, name: "Trash" }).waitFor();
    await pause(800);
    await caption(page, "Trash is the last chip, and the row scrolls so it stays in view", 2600);
  } else {
    await go(page, "8", "Trash");
    await caption(page, "G then 8 is still Trash: the older keys did not move", 2400);
  }
  await pause(1000);
}
