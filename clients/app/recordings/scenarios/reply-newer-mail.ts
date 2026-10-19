import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// A reply keeps answering the message its draft was written for when newer
// mail arrives in the conversation (campbooks-cloud#220). On the stack the
// `reply-dialog` scenario describes (its header: inbox.ts's settings, a
// `decide` model and the fake `complete` model):
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record reply-newer-mail \
//     --base-url http://localhost:5181
//
// While the dialog shows Scout's draft for Maya's last message, the fake
// Google delivers Leo's answer in the same thread
// (`POST /fake/deliver?reply=q4`, at FAKE_GOOGLE_URL, default
// http://127.0.0.1:9071). The dialog says a newer message arrived and keeps
// the quote of Maya's; "Answer the newer one" writes the draft again for
// Leo's. Nothing is sent: the dialog is closed with Esc. Fictional people and
// mail only.

const SUBJECT = "Q4 plan: can you review by Friday?";
const FAKE_GOOGLE = process.env["FAKE_GOOGLE_URL"] ?? "http://127.0.0.1:9071";

export default async function replyNewerMail({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}/inbox/primary`);
  const list = page.getByRole("region", { name: "Conversations" });
  await list.getByText(SUBJECT).first().click();
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: SUBJECT }).first().waitFor();

  // Reply, and pick Scout's first likely reply: the draft answers Maya.
  await page
    .getByRole("button", { name: /^Reply(?! all)/u })
    .first()
    .click();
  const dialog = page.getByRole("dialog", { name: /^Reply to/u });
  await dialog.waitFor();
  const first = dialog.locator("[data-dialog-choice]").first();
  await first.waitFor();
  await first.click();
  await dialog.getByRole("textbox", { name: "Your reply" }).waitFor();
  await caption(page, "Scout's draft answers Maya's last message", 2200);

  // Leo answers in the same thread while the draft is open.
  await page.request.post(`${FAKE_GOOGLE}/fake/deliver?reply=q4`);
  const note = dialog.getByText(/A newer message from Leo Brandt arrived/u);
  // The stream's next incremental sync picks it up (minutes apart in dev).
  for (let attempt = 0; attempt < 240; attempt += 1) {
    if (await note.isVisible()) {
      break;
    }
    await pause(1000);
  }
  await note.waitFor();
  await caption(page, "Newer mail arrived: the reply still answers the one quoted", 2600);

  // Answer Leo's instead: Scout writes the draft again for it.
  await dialog.getByRole("button", { name: "Answer the newer one" }).click();
  await dialog
    .getByText(/Leo Brandt/u)
    .first()
    .waitFor();
  await dialog.getByRole("textbox", { name: "Your reply" }).waitFor();
  await caption(page, "Answer the newer one: the draft and the quote follow it", 2600);
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
}
