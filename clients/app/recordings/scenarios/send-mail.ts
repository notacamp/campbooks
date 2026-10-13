import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Replying, forwarding and writing a new message on the real stack: the Vite
// dev server, the real server (API, BFF and worker) on a scratch database,
// and the fake Google with its demo Gmail mailbox, which stores what is sent
// as SENT and delivers nothing. Nothing is intercepted: drafts are saved by
// the conversations module, the send leaves after its undo window through the
// connectors module's Gmail adapter, and the sent message comes back into
// the thread.
//
//   pnpm --dir clients fake-google --port 9081
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5191 \
//   CAMPBOOKS_BIND_ADDR=127.0.0.1:8201 CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9081 CAMPBOOKS_BLOBS__PATH=/tmp/blobs \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- all
//   CAMPBOOKS_API_URL=http://127.0.0.1:8201 pnpm --dir clients/app exec vite --port 5191
//   pnpm --dir clients record send-mail --base-url http://localhost:5191
//
// Fictional people and addresses only (`example.com`); no token is shown.

/** A caption pinned to the bottom of the page, so the video explains itself. */
async function caption(page: Page, text: string, ms = 1600): Promise<void> {
  // A script string: the recordings are type-checked without the DOM types.
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-caption") ?? document.createElement("div");
    box.id = "recording-caption";
    box.textContent = ${JSON.stringify(text)};
    const phone = window.innerWidth < 768;
    Object.assign(box.style, {
      position: "fixed", zIndex: "2147483647", pointerEvents: "none",
      left: phone ? "8px" : "auto", right: "8px", top: phone ? "4px" : "auto",
      bottom: phone ? "auto" : "16px", maxWidth: phone ? "none" : "520px",
      padding: phone ? "6px 10px" : "10px 14px", borderRadius: "10px",
      background: "rgba(20, 20, 24, 0.9)", color: "#fff",
      font: phone ? "500 12px/1.35 system-ui, sans-serif" : "500 15px/1.4 system-ui, sans-serif",
    });
    document.body.append(box);
  })()`);
  await pause(ms);
}

/** Call the API through the BFF from the page, as the app does. */
async function api(page: Page, method: string, path: string): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)},
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
  }).then((response) => response.status === 204 ? null : response.json())`);
}

/** Sign up with the fake Google, make a workspace and connect its Gmail. */
async function setUp(page: Page): Promise<string> {
  await page.goto("/");
  await page.waitForURL(/\/sign-in\?request=/u);
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Rivera Studio");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("button", { name: /^Connect/u }).click();
  await page.waitForURL(/\/w\//u);
  const { workspaces } = (await api(page, "GET", "/bff/api/v1/workspaces")) as {
    workspaces: { id: string }[];
  };
  const workspace = workspaces[0]?.id ?? "";
  // The first sync: the backfill reads the demo mailbox.
  for (let attempt = 0; attempt < 90; attempt += 1) {
    const list = (await api(page, "GET", `/bff/api/v1/w/${workspace}/conversations`)) as {
      conversations?: unknown[];
    };
    if ((list.conversations?.length ?? 0) >= 7) {
      break;
    }
    await pause(1000);
  }
  return workspace;
}

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
  const workspace = await setUp(page);
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
  await caption(page, "Send again, and let it go", 1200);
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
