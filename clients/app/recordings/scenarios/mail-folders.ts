import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// Sent, Archive, Spam and Trash under the streams (campbooks-cloud#240) on
// the real stack: start it as inbox.ts says. No AI is involved, and nothing
// is sent: every change goes to the fake Google's demo mailbox only.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record mail-folders \
//     --base-url http://localhost:5181
//
// Fictional senders on `example` domains only.

const STREAMS = ["primary", "updates", "receipts", "newsletters"] as const;
const BERTH = "Your berth for next season";
const TIDES = "Tide tables for the season";
const LEDGER = "Five tax dates before year end";

/** Desktop and phone differ only where the screens do: one pane or two. */
function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** The stream a conversation with `subject` is in, once the first sync has it. */
async function streamOf(page: Page, workspace: string, subject: string): Promise<string> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    for (const stream of STREAMS) {
      const list = (await api(
        page,
        "GET",
        `/bff/api/v1/w/${workspace}/conversations?stream=${stream}`,
      )) as { conversations?: { subject: string }[] };
      if (list.conversations?.some((c) => c.subject === subject) === true) {
        return stream;
      }
    }
    await pause(1000);
  }
  throw new Error(`no conversation "${subject}" in the demo mailbox`);
}

/** Open the row with `subject` in the list on screen and wait for the reader. */
async function open(page: Page, subject: string) {
  const list = page.getByRole("region", { name: "Conversations" });
  await list.getByText(subject).first().click();
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: subject }).waitFor();
  return reader;
}

/** G then `digit`: a stream (1 to 4) or a view (5 to 8). */
async function go(page: Page, digit: string, heading: string): Promise<void> {
  await page.keyboard.press("g");
  await page.keyboard.press(digit);
  await page.getByRole("heading", { level: 1, name: heading }).waitFor();
}

export default async function mailFolders({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  // The recorded page starts blank: `api` fetches relative to the app's origin.
  await page.goto(`/w/${workspace}`);
  const berth = await streamOf(page, workspace, BERTH);
  const tides = await streamOf(page, workspace, TIDES);
  const ledger = await streamOf(page, workspace, LEDGER);

  // 1. The views sit under the streams.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await caption(
    page,
    phone
      ? "After the stream chips, a divider, then Sent, Archive, Spam and Trash"
      : "Under the streams, a quiet divider, then Sent, Archive, Spam and Trash (G then 5 to 8)",
    2600,
  );

  // 2. Sent: to whom, and whether they replied.
  await go(page, "5", "Sent");
  await page.getByText("To Maya Okafor").first().waitFor();
  await caption(page, "Sent: every conversation you wrote in, by your latest message", 2400);
  await caption(page, "Each row says who it went to, and Replied once they answered", 2400);

  // 3. Done, then Archive and Move to Inbox (E).
  await page.goto(`/w/${workspace}/inbox/${berth}`);
  await open(page, BERTH);
  await page.keyboard.press("e");
  await page.getByText(`Done · ${BERTH}`).waitFor();
  await go(page, "6", "Archive");
  await open(page, BERTH);
  await caption(page, "Archive: out of the inbox. E moves it back", 2000);
  await page.keyboard.press("e");
  await page.getByText(`Moved to Inbox · ${BERTH}`).waitFor();
  await caption(page, "Moved to Inbox, with Undo", 1800);

  // 4. Delete (#) with Undo, then Trash and Restore (#).
  await page.goto(`/w/${workspace}/inbox/${tides}`);
  await open(page, TIDES);
  await page.keyboard.press("#");
  await page.getByText(`Deleted · ${TIDES}`).waitFor();
  await caption(page, "Delete is #, and the toast can take it back", 2000);
  await go(page, "8", "Trash");
  const trashed = await open(page, TIDES);
  await trashed.getByText("This conversation is in Trash.").waitFor();
  await caption(page, "In Trash: no reply box. # restores it where it was", 2200);
  await page.keyboard.press("#");
  await page.getByText(`Restored · ${TIDES}`).waitFor();
  await pause(1200);

  // 5. Report spam (!), then Spam: the banner, and no images.
  await page.goto(`/w/${workspace}/inbox/${ledger}`);
  await open(page, LEDGER);
  await page.keyboard.press("!");
  await page.getByText(`Reported as spam · ${LEDGER}`).waitFor();
  await go(page, "7", "Spam");
  const spam = await open(page, LEDGER);
  await spam.getByText("This conversation is in Spam.").waitFor();
  await spam.getByText("Images never load in Spam.").first().waitFor();
  await caption(
    page,
    "In Spam, images never load, not even from a sender you trust, and Scout stays out",
    2800,
  );

  // 6. Not spam (!): back in the inbox.
  await page.keyboard.press("!");
  await page.getByText(`Not spam · ${LEDGER}`).waitFor();
  await caption(page, "! again: not spam, back in the inbox", 2000);
  await page.goto(`/w/${workspace}/inbox/${ledger}`);
  await page.getByText(LEDGER).first().waitFor();
  await pause(1200);
}
