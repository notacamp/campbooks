import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { api, caption, signedIn } from "./inbox.ts";

// What you do in the inbox happens in Gmail too (connectors write-back), on
// the real stack with the fake Google, as the `inbox` scenario runs it (see
// its header for the commands). The fake's `GET /fake/mailbox` shows what the
// demo Gmail holds; nothing is intercepted.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record gmail-writeback \
//     --base-url http://localhost:5181
//
// Fictional person and mail only; no token is ever shown.

const fakeGoogle = process.env["FAKE_GOOGLE_URL"] ?? "http://127.0.0.1:9071";
const subject = "Q4 plan: can you review by Friday?";

/** The labels the fake Gmail holds on the message with `subject`. */
async function gmailLabels(page: Page): Promise<string[]> {
  const response = await page.request.get(`${fakeGoogle}/fake/mailbox`);
  const { mailboxes } = (await response.json()) as {
    mailboxes: { subject: string; labels: string[] }[][];
  };
  // The newest account is this recording's (each run signs up afresh).
  return mailboxes.at(-1)?.find((message) => message.subject === subject)?.labels ?? [];
}

/** A panel showing what Gmail holds, so the video shows the provider side. */
async function showGmail(page: Page, labels: string[]): Promise<void> {
  const text = `Gmail (fake Google): "${subject}" has ${labels.join(", ")}`;
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

export default async function gmailWriteback({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(subject).first().waitFor();
  await showGmail(page, await gmailLabels(page));
  await caption(page, "Gmail holds the Q4 plan in its inbox (INBOX)", 2200);

  await page.getByText(subject).first().click();
  await page.getByRole("heading", { name: subject }).waitFor();
  await caption(page, phone ? "Done archives it" : "Done (E) archives it");
  await (phone
    ? page
        .getByRole("group", { name: "Conversation actions" })
        .getByRole("button", { name: "Done" })
        .click()
    : page.keyboard.press("e"));

  // The connector writes it back on the stream's next run.
  let labels = await gmailLabels(page);
  for (let attempt = 0; attempt < 60 && labels.includes("INBOX"); attempt += 1) {
    await pause(500);
    labels = await gmailLabels(page);
  }
  await showGmail(page, labels);
  await caption(page, "…and Gmail archived it too: INBOX is gone", 2600);

  // The next sync reads Gmail's record of that write and brings nothing back.
  const { connections } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/connections`)) as {
    connections: { id: string }[];
  };
  await api(
    page,
    "POST",
    `/bff/api/v1/w/${workspace}/connections/${connections[0]?.id ?? ""}/sync`,
  );
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await pause(2500);
  await showGmail(page, await gmailLabels(page));
  await caption(page, "Synced again: it stays archived, nothing echoes back", 2600);
}
