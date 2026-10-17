import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// "Always show images from <sender>" (campbooks-cloud#200) on the real stack:
// start it as inbox.ts says. No AI is involved. The demo mailbox holds The
// Weekly Ledger's newsletter, whose Gmail results pass DMARC, and a message
// pretending to be the Ledger that fails it. Fictional senders on `example`
// domains only; nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record trusted-images \
//     --base-url http://localhost:5181
//
// Locators are scoped to the reader (`main` "Conversation"), the dialog, or
// the rule's row in Scout's memory.

const NEWSLETTER = "Five tax dates before year end";
const SPOOF = "Your Ledger subscription could not be renewed";
const STREAMS = ["primary", "updates", "receipts", "newsletters"] as const;

/** Where a conversation with `subject` is: its stream and id. */
async function find(
  page: Page,
  workspace: string,
  subject: string,
): Promise<{ stream: string; id: string }> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    for (const stream of STREAMS) {
      const list = (await api(
        page,
        "GET",
        `/bff/api/v1/w/${workspace}/conversations?stream=${stream}`,
      )) as { conversations?: { id: string; subject: string }[] };
      const found = list.conversations?.find((c) => c.subject === subject);
      if (found !== undefined) {
        return { stream, id: found.id };
      }
    }
    await pause(1000);
  }
  throw new Error(`no conversation "${subject}" in the demo mailbox`);
}

/** Open a conversation in the reader. */
async function read(page: Page, workspace: string, subject: string) {
  const { stream, id } = await find(page, workspace, subject);
  await page.goto(`/w/${workspace}/inbox/${stream}/${id}`);
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: subject }).waitFor();
  return reader;
}

export default async function trustedImages({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  // The recorded page starts blank: `api` fetches relative to the app's origin.
  await page.goto(`/w/${workspace}`);

  let reader = await read(page, workspace, NEWSLETTER);
  await reader
    .getByText(/remote image hidden/u)
    .first()
    .waitFor();
  await caption(page, "Remote images are held back until you choose Show images", 2400);

  await reader.getByRole("button", { name: /^Always show images from The Weekly Ledger/u }).click();
  const dialog = page.getByRole("dialog", {
    name: "Always show images from hello@weekly-ledger.example",
  });
  await dialog.waitFor();
  await caption(page, "Trust the sender: a smart rule you check first, with no AI", 2600);
  await caption(
    page,
    "Only when the mail passes its sender checks; tracking pixels stay removed",
    2600,
  );
  await dialog.getByRole("button", { name: /Save smart rule/u }).click();
  await reader.getByText(/Images shown: you trust The Weekly Ledger/u).waitFor();
  await caption(page, "Its images now show on their own; Undo takes the rule back", 2600);

  reader = await read(page, workspace, SPOOF);
  await reader
    .getByText(/remote image hidden/u)
    .first()
    .waitFor();
  await caption(
    page,
    "Mail pretending to be the Ledger fails DMARC: its images stay held back",
    2800,
  );

  // Scout's memory → Smart rules: remove it, and "Show images" is back.
  await page.goto(`/w/${workspace}/settings/memory`);
  const row = page.getByRole("listitem").filter({ hasText: /its images always show/u });
  await row.first().waitFor();
  await caption(page, "The rule lives in Scout's memory, like every smart rule", 2200);
  await row.first().getByRole("button", { name: "Remove" }).click();
  await row.first().getByRole("button", { name: "Remove" }).click();
  await pause(1200);

  reader = await read(page, workspace, NEWSLETTER);
  await reader.getByRole("button", { name: /^Show images/u }).waitFor();
  await caption(page, "Removed: the Ledger's images are held back again", 2400);
}
