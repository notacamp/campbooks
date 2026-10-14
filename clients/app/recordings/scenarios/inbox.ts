import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// The inbox on the real stack: the Vite dev server, the real server (API,
// BFF and worker) on a scratch database, and the fake Google with its demo
// Gmail mailbox (`scripts/fake-provider.ts`, `scripts/fake-gmail.ts`). Nothing is intercepted: the
// mail is synced by the connectors module, sorted by the conversations
// module, and read through the BFF.
//
//   pnpm --dir clients fake-google --port 9071
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5181 \
//   CAMPBOOKS_BIND_ADDR=127.0.0.1:8191 CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9071 CAMPBOOKS_BLOBS__PATH=/tmp/blobs \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- all
//   CAMPBOOKS_API_URL=http://127.0.0.1:8191 pnpm --dir clients/app exec vite --port 5181
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record inbox --base-url http://localhost:5181
//
// Sign-up, the first workspace and connecting the mailbox are other slices'
// screens; the scenario goes through them quickly, then records the inbox.
// Fictional person and mail only; no token is ever shown.

/** A caption pinned to the bottom of the page, so the video explains itself. */
export async function caption(page: Page, text: string, ms = 1600): Promise<void> {
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
export async function api(page: Page, method: string, path: string): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)},
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
  }).then((response) => response.status === 204 ? null : response.json())`);
}

/**
 * Sign up through the sign-in screens with the fake Google, create a
 * workspace and connect the mailbox from sign-up, then wait for the first
 * sync to bring the demo mail in.
 */
async function setUp(page: Page): Promise<string> {
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Rivera Studio");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("heading", { name: /will be connected to this workspace/u }).waitFor();
  await page.keyboard.press("Enter");
  // Home: "Your mail" until the inbox exists, then what needs you.
  await page.waitForURL(/\/w\/[^/]+$/u);
  await page.getByRole("heading", { level: 1 }).waitFor();
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

/**
 * Run `setUp` in a context of its own (not recorded) and hand its session
 * cookie to the recorded page, so the video starts in the inbox.
 */
export async function signedIn(page: Page): Promise<string> {
  const browser = page.context().browser();
  if (browser === null) {
    return setUp(page);
  }
  // The recorder's --base-url, repeated (a context does not expose its own).
  const baseURL = process.env["INBOX_BASE_URL"] ?? "http://localhost:5173";
  const setup = await browser.newContext({ baseURL });
  try {
    const workspace = await setUp(await setup.newPage());
    await page.context().addCookies((await setup.storageState()).cookies);
    return workspace;
  } finally {
    await setup.close();
  }
}

export default async function inbox({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const shots = process.env["INBOX_SHOTS"];
  const shot = async (name: string) => {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/${name}-${phone ? "375" : "desktop"}.png` });
    }
  };

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText("Q4 plan: can you review by Friday?").first().waitFor();
  await caption(page, "The inbox is streams: Primary, Updates, Receipts, Newsletters");
  await shot("01-primary");
  await caption(
    page,
    "Nothing moves by itself yet: Campbooks' sorting shows as a suggestion on the row",
    2200,
  );

  // Open the newsletter: a suggestion with its reasons, remote images blocked.
  if (phone) {
    await page
      .getByRole("button", { name: /The Weekly Ledger/u })
      .first()
      .click();
  } else {
    await caption(page, "J and K move, Enter opens: every action shows its key");
    for (let index = 0; index < 4; index += 1) {
      await page.keyboard.press("j");
      await pause(350);
    }
    await page
      .getByRole("button", { name: /The Weekly Ledger/u })
      .first()
      .click();
  }
  await page.getByRole("heading", { name: "Five tax dates before year end" }).waitFor();
  await caption(page, "The reader: Campbooks' read is a suggestion");
  await page.getByText("How Campbooks decided").click();
  await caption(page, "How Campbooks decided shows the stored reasons behind it", 2200);
  await shot("02-reader-why");
  await page.getByText("How Campbooks decided").click();

  await page.mouse.wheel(0, 300);
  await caption(
    page,
    "The body: sanitised on the server, shown in a sandboxed frame; remote images blocked",
    2400,
  );
  await shot("03-images-blocked");
  await page
    .getByRole("button", { name: /Show images/u })
    .first()
    .click();
  await pause(800);
  await caption(page, "Show images loads them for this message (tracking pixels stay out)", 2200);
  await shot("04-images-shown");

  // Move to the suggested stream (1), then Undo.
  await page.mouse.wheel(0, -600);
  await caption(page, "Take the suggestion: Move to Newsletters (1)");
  await (phone
    ? page.getByRole("button", { name: "Move to Newsletters" }).click()
    : page.keyboard.press("1"));
  await pause(900);
  await shot("05-moved");
  await caption(page, "Every change can be undone: the toast, or Z", 1800);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await pause(1200);

  // Back to the list, open the Q4 thread: newest message open, older folded.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText("Q4 plan: can you review by Friday?").first().click();
  await page.getByRole("heading", { name: "Q4 plan: can you review by Friday?" }).waitFor();
  await caption(page, "A thread: newest first, older messages folded, attachments listed", 2200);
  await shot("06-thread");
  await page
    .getByRole("button", { name: /Sam Rivera/u })
    .first()
    .click();
  await pause(900);

  // Done (archive) with Undo.
  await caption(page, phone ? "Done archives it" : "Done (E) archives it, Z brings it back");
  await (phone
    ? page
        .getByRole("group", { name: "Conversation actions" })
        .getByRole("button", { name: "Done" })
        .click()
    : page.keyboard.press("e"));
  await pause(900);
  await shot("07-done-toast");
  await (phone ? page.getByRole("button", { name: /^Undo/u }).click() : page.keyboard.press("z"));
  await pause(1200);

  // Move a conversation with M, then the other streams.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText("Your invoice CH-20418 for €148.00").first().click();
  await page.getByRole("heading", { name: "Your invoice CH-20418 for €148.00" }).waitFor();
  await caption(page, phone ? "Move puts it in another stream" : "M: move it to another stream");
  await (phone
    ? page.getByRole("button", { name: "Move" }).first().click()
    : page.keyboard.press("m"));
  await pause(900);
  await shot("08-move-menu");
  await page.getByRole("button", { name: /^Receipts/u }).click();
  await pause(1000);
  await caption(page, phone ? "Now in Receipts" : "Now in Receipts: G then 3 goes there");
  if (phone) {
    await page.goto(`/w/${workspace}/inbox/receipts`);
  } else {
    await page.keyboard.press("g");
    await page.keyboard.press("3");
  }
  await page.getByRole("heading", { level: 1, name: "Receipts" }).waitFor();
  await pause(1200);
  await shot("09-receipts");
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await caption(
    page,
    "Live: new mail arrives at Google; the next sync and the change feed bring it in",
    1200,
  );
  await page.request.post("http://127.0.0.1:9071/fake/deliver");
  // Sync now instead of waiting for the next scheduled run.
  const { connections } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/connections`)) as {
    connections: { id: string }[];
  };
  await api(
    page,
    "POST",
    `/bff/api/v1/w/${workspace}/connections/${connections[0]?.id ?? ""}/sync`,
  );
  await page.getByText("VAT Q3: two receipts missing").first().waitFor({ timeout: 120_000 });
  await caption(page, "VAT Q3 from Rui arrived in Primary", 1800);
  await shot("10-live");
}
