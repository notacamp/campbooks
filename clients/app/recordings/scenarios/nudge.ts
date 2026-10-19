import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// Waiting on others and the nudge (campbooks-cloud#293) on the stack the
// `reply-dialog` scenario describes (inbox.ts's settings, a `decide` model
// that answers `next_move` and `awaited`, and the fake `complete` model,
// which writes the nudge with canned, fictional text), with the fake
// Google's `--waits` demo mail:
//
//   pnpm --dir clients fake-google --port 9071 --waits
//   pnpm --dir clients fake-complete
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete … cargo run -p campbooks-server -- all
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record nudge \
//     --base-url http://localhost:5181
//
// João Matos asked Ana Costa, copying Sam, for the revised floor plan eight
// days ago, so Scout's clock has already turned the wait into a nudge; Rui
// Matos promised a contract by Friday, a wait. The nudge goes to the fake
// mailbox only. Then the fake Google delivers Ana's answer
// (`POST /fake/deliver?reply=ana`, at FAKE_GOOGLE_URL), and the wait ends:
// "Heard back" in Done today. Fictional people and mail only.

const FAKE_GOOGLE = process.env["FAKE_GOOGLE_URL"] ?? "http://127.0.0.1:9071";
const ANA = "Ana Costa hasn’t written yet";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/**
 * Open Home and wait for `text`, for at most `seconds`: Home follows the
 * change feed, so a need that changes arrives without a reload. One reload
 * at the end covers a feed that dropped.
 */
async function homeShows(page: Page, workspace: string, text: string, seconds = 300) {
  const home = `/w/${workspace}`;
  if (!page.url().endsWith(home)) {
    await page.goto(home);
  }
  await page.getByRole("heading", { level: 1 }).waitFor();
  try {
    await page
      .getByText(text)
      .first()
      .waitFor({ timeout: seconds * 1000 });
  } catch {
    await page.goto(home);
    await page.getByText(text).first().waitFor({ timeout: 10_000 });
  }
}

/** Sync the mailbox now instead of waiting for the next scheduled run. */
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

export default async function nudge({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeShows(page, workspace, ANA);
  await caption(page, "João asked Ana, copying you, to send you the floor plan", 2400);
  await caption(page, "Ana still hasn't written, so Scout puts it under Nudge", 2400);

  // Ana's item: Nudge on 1 (the button on a phone).
  await page.getByText(ANA).first().click();
  await caption(page, "Nudge, Heard from them, Snooze, or Not waiting", 2000);
  await (phone
    ? page
        .getByRole("button", { name: /^Nudge Ana/u })
        .first()
        .click()
    : page.keyboard.press("1"));

  const dialog = page.getByRole("dialog", { name: "Nudge Ana" });
  await dialog.waitFor();
  await caption(page, "Scout writes a few lines to Ana, in the same thread, copying João", 2400);
  const text = dialog.getByRole("textbox", { name: "Your nudge" });
  await text.waitFor();
  await pause(1600);
  await dialog.getByRole("button", { name: /Shorter/u }).click();
  await caption(page, "Yours to edit, or make shorter. Nothing is sent before Send", 2200);
  await pause(1200);

  await (phone
    ? dialog.getByRole("button", { name: /^Send/u }).click()
    : page.keyboard.press("Control+Enter"));
  await caption(page, "Sent, with Undo until it leaves. The item folds away", 2400);
  await pause(1200);

  // Once the nudge leaves, Scout waits on Ana again: Waiting on others.
  const waiting = page.getByRole("listitem", { name: "Waiting on Ana Costa" });
  await waiting.waitFor({ timeout: 120_000 });
  await waiting.scrollIntoViewIfNeeded();
  await caption(page, "Scout waits on Ana again and says when it checks", 2400);

  // Ana writes back, in a thread of her own.
  await page.request.post(`${FAKE_GOOGLE}/fake/deliver?reply=ana`);
  await syncNow(page, workspace);
  await homeShows(page, workspace, "Heard back · Revised floor plan", 180);
  if (phone) {
    await page.getByText("Heard back · Revised floor plan").first().scrollIntoViewIfNeeded();
  }
  await caption(page, "Ana wrote, so Scout stopped waiting: it is in Done today", 2600);
}
