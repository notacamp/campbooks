import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// Replying, forwarding and writing a new message on the real stack, as the
// `inbox` scenario runs it (see its header for the commands: the fake Google
// of `scripts/fake-provider.ts` with the demo Gmail of `scripts/fake-gmail.ts`,
// which stores what is sent as SENT and delivers nothing). Nothing is
// intercepted: drafts are saved by the conversations module, the send leaves
// after its undo window through the connectors module's Gmail adapter, and
// the sent message comes back into the thread.
//
//   INBOX_BASE_URL=http://localhost:5191 pnpm --dir clients record send-mail \
//     --base-url http://localhost:5191
//
// Fictional people and addresses only (`example.com`); no token is shown.

/** Desktop and phone differ only where the screens do: one pane or two. */
function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Press a reader action: its key on desktop, its button on a phone. */
async function act(page: Page, phone: boolean, name: string, key: string): Promise<void> {
  await (phone
    ? page.getByRole("button", { name, exact: true }).last().click()
    : page.keyboard.press(key));
}

const Q4 = "Q4 plan: can you review by Friday?";

export default async function sendMail({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(Q4).first().click();
  await page.getByRole("heading", { name: Q4 }).waitFor();
  await caption(page, phone ? "A thread from Maya: Reply" : "A thread from Maya: R replies");
  await act(page, phone, "Reply", "r");

  const composer = page.getByRole("region", { name: /^Reply to/u });
  await composer.waitFor();
  await caption(page, "The composer opens under the thread: From your mailbox, To filled in");
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.pressSequentially("Thanks Maya, the timeline works: ", { delay: 25 });
  // Bold with nothing selected puts the marks around what comes next.
  await (phone
    ? composer.getByRole("button", { name: "Bold" }).click()
    : page.keyboard.press("ControlOrMeta+b"));
  await body.pressSequentially("two weeks later", { delay: 25 });
  await page.keyboard.press("End");
  await body.pressSequentially(" is fine.", { delay: 25 });
  await caption(page, phone ? "Bold, lists: basic formatting" : "⌘B bold, lists: basic formatting");
  await composer.getByRole("button", { name: "Show quoted text" }).click();
  await caption(page, "The quoted history sits folded under your text", 1800);
  await composer.getByRole("button", { name: "Hide quoted text" }).click();
  await composer.getByText("Saved").waitFor();
  await caption(page, "Saved as you type");

  await caption(page, phone ? "Send" : "⌘↵ sends");
  await (phone
    ? composer.getByRole("button", { name: /^Send/u }).click()
    : body.press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to Maya Okafor/u).waitFor();
  await caption(page, "It waits 10 seconds before leaving: Undo takes it back", 1200);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await composer.waitFor();
  await caption(page, "Back to the draft, nothing sent", 1800);

  await (phone
    ? composer.getByRole("button", { name: /^Send/u }).click()
    : body.press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to Maya Okafor/u).waitFor();
  await caption(page, "Send again, and let it go: Undo stays up for the whole window", 2000);
  await page.getByText(/^Sending to Maya Okafor/u).waitFor({ state: "hidden", timeout: 20_000 });
  // Once it has left (the release runs as the window ends).
  await pause(4000);
  await caption(page, "Z after the window has passed…", 1000);
  await page.keyboard.press("z");
  await page.getByText("Too late to undo: it was sent.").waitFor();
  await caption(page, "…says so: it was sent", 1800);
  // The sent message, in the thread (its body in the sandboxed frame).
  await page
    .locator('iframe[title*="Sam Rivera"]')
    .last()
    .contentFrame()
    .getByText(/the timeline works/u)
    .waitFor({ timeout: 60_000 });
  await caption(page, "Sent through Gmail: the reply is in the thread, once", 2400);
  await page.mouse.wheel(0, 600);
  await pause(1200);

  // Forward Maya's first message, with its attachment.
  await page.mouse.wheel(0, -2000);
  await caption(page, phone ? "Forward" : "F forwards");
  await act(page, phone, "Forward", "f");
  const forward = page.getByRole("region", { name: "Forward" });
  await forward.waitFor();
  await caption(
    page,
    "Forward: the subject, the quoted message and its attachments come along",
    2200,
  );
  const to = forward.getByRole("textbox", { name: "To" });
  await to.pressSequentially("leo@example.com", { delay: 30 });
  await to.press("Enter");
  await caption(page, "Recipients are chips", 1500);
  await (phone
    ? forward.getByRole("button", { name: /^Send/u }).click()
    : forward.getByRole("textbox", { name: "Message" }).press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to leo@example.com/u).waitFor();
  await pause(1500);

  // A new message.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await caption(page, phone ? "New message" : "C writes a new message");
  await (phone
    ? page.getByRole("button", { name: /New message/u }).click()
    : page.keyboard.press("c"));
  const fresh = page.getByRole("region", { name: "New message" });
  await fresh.waitFor();
  const freshTo = fresh.getByRole("textbox", { name: "To" });
  await freshTo.pressSequentially("Ana Lopes <ana@example.com>,", { delay: 25 });
  await fresh.getByRole("button", { name: "Cc Bcc" }).click();
  await fresh
    .getByRole("textbox", { name: "Cc", exact: true })
    .pressSequentially("joana@example.com", {
      delay: 25,
    });
  await fresh.getByRole("textbox", { name: "Subject" }).fill("Workshop on 20 October");
  const freshBody = fresh.getByRole("textbox", { name: "Message" });
  await freshBody.fill("Hi Ana,\n\nCould you book two seats?\n- morning session\n- lunch\n\nSam");
  await caption(page, "From your mailbox, To, Cc and Bcc, a subject, a list", 2000);
  await (phone
    ? fresh.getByRole("button", { name: /^Send/u }).click()
    : freshBody.press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to Ana Lopes/u).waitFor();
  await caption(page, "Sent after its undo window, back in the inbox", 2400);
}
