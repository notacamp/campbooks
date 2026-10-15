import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Settings → Mailboxes on the real stack (campbooks-cloud#68): the Vite dev
// server, the real server and BFF on a scratch database, and the dev-only
// fake Google and fake Zoho of `scripts/fake-provider.ts`, whose demo Gmail
// and Zoho Mail mailboxes the connectors module really syncs. Nothing is
// intercepted. `POST /fake/expire` on the fake Zoho withdraws its grants, as
// someone revoking the app at Zoho would.
//
//   pnpm --dir clients fake-google --port 9171
//   pnpm --dir clients fake-zoho --port 9172
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5281 \
//   CAMPBOOKS_BIND_ADDR=127.0.0.1:8291 CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9171 \
//   CAMPBOOKS_ZOHO__CLIENT_ID=demo-zoho-client CAMPBOOKS_ZOHO__CLIENT_SECRET=demo-zoho-secret \
//   CAMPBOOKS_ZOHO__BASE_URL=http://127.0.0.1:9172 CAMPBOOKS_BLOBS__PATH=/tmp/blobs \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- all
//   CAMPBOOKS_API_URL=http://127.0.0.1:8291 pnpm --dir clients/app exec vite --port 5281
//   pnpm --dir clients record mailboxes --base-url http://localhost:5281
//
// Sign up with Google and forget the mailbox from sign-up: Home's "Connect
// your mail" opens Settings → Mailboxes. Connect Gmail, then Zoho Mail (also
// as a way to sign in); Zoho withdraws the grant, so it needs you:
// Reconnect. Disconnect Zoho keeping its history: its inbox reads as kept
// history and Zoho, a way in only by connecting it, leaves Ways to sign in.
// Disconnect Gmail deleting its mail: it leaves the inbox. Fictional person
// and mail only; no token is ever shown.

const fakeZoho = process.env["FAKE_ZOHO_URL"] ?? "http://127.0.0.1:9172";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Allow on the fake provider's consent pages, as many as it asks. */
async function allowAll(page: Page, provider: string): Promise<void> {
  await page.getByText(`Fake ${provider} · development only`).waitFor();
  await caption(page, `${provider}'s consent (a dev-only fake): Allow`, 1400);
  // The fakes listen on 127.0.0.1; the app on localhost.
  while (new URL(page.url()).hostname === "127.0.0.1") {
    await page.getByRole("link", { name: "Allow" }).click();
    await page.waitForLoadState();
    await pause(500);
  }
}

async function openMailboxes(page: Page, phone: boolean): Promise<void> {
  if (phone) {
    await page.getByRole("link", { name: "Settings" }).last().click();
    await page.getByRole("button", { name: "Mailboxes", exact: true }).click();
  } else {
    await page.getByRole("link", { name: "Settings" }).first().click();
    await page.getByRole("link", { name: "Mailboxes", exact: true }).click();
  }
  await page.getByRole("heading", { level: 1, name: "Mailboxes" }).waitFor();
}

/** Call the API through the BFF from the page, as the app does. */
async function call<T>(page: Page, method: string, path: string): Promise<T> {
  return page.evaluate<T>(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)}, headers: { "x-campbooks-csrf": "1" },
  }).then((response) => response.status === 204 ? null : response.json())`);
}

/** Wait until `done` holds, re-checking every second (at most `seconds`). */
async function until(done: () => Promise<boolean>, seconds = 120): Promise<void> {
  for (let left = seconds; left > 0 && !(await done()); left -= 1) {
    await pause(1000);
  }
}

/** Open a conversation of the Primary stream by its subject. */
async function openConversation(page: Page, workspace: string, subject: string): Promise<void> {
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(subject).first().click();
  await page.getByRole("heading", { name: subject }).waitFor();
}

/** Press `key` on desktop, or click `button` on a phone. */
async function act(page: Page, phone: boolean, key: string, button: RegExp): Promise<void> {
  await (phone ? page.getByRole("button", { name: button }).click() : page.keyboard.press(key));
}

export default async function mailboxes({ page }: ScenarioContext): Promise<void> {
  const phone = isPhone(page);
  const QUOTE = "Orçamento para a montra de novembro";
  const Q4 = "Q4 plan: can you review by Friday?";

  // Sign up with Google: an account and a workspace. The mailbox from
  // sign-up is forgotten, so Home offers "Connect your mail".
  await page.goto("/");
  await caption(page, "Sign up with Google");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await allowAll(page, "Google");
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Rivera Studio");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("button", { name: /Not now/u }).click();
  await page.getByRole("button", { name: "Don’t connect it" }).click();
  await page.getByRole("button", { name: "Forget it" }).click();
  await page.getByRole("heading", { name: "Connect your mail" }).waitFor();
  await caption(page, "Home: no mailbox yet. “Connect your mail”…", 1800);
  await page.getByRole("link", { name: /Connect your mail/u }).click();
  await page.getByRole("heading", { level: 1, name: "Mailboxes" }).waitFor();
  await caption(page, "…opens Settings → Mailboxes", 1800);
  const { workspaces } = await call<{ workspaces: { id: string }[] }>(
    page,
    "GET",
    "/bff/api/v1/workspaces",
  );
  const workspace = workspaces[0]?.id ?? "";

  // Connect Gmail, then Zoho Mail, through the front door.
  await page.getByRole("checkbox", { name: "Also use this account to sign in" }).waitFor();
  await caption(
    page,
    "“Also use this account to sign in”: ticked for your own; untick it for support@",
    2400,
  );
  if (!phone) {
    await caption(page, "⇧C jumps to Connect", 1000);
    await page.keyboard.press("Shift+C");
    await pause(500);
  }
  await page.getByRole("button", { name: /Connect Gmail/u }).click();
  await allowAll(page, "Google");
  await page.getByText(/^Gmail .+ is connected$/u).waitFor();
  await page.getByRole("button", { name: /Connect Zoho Mail/u }).click();
  await allowAll(page, "Zoho");
  await page.getByText(/^Zoho Mail .+ is connected$/u).waitFor();
  await caption(page, "Back on Mailboxes: Gmail and Zoho Mail, syncing, connected by you", 2400);

  // Zoho withdraws the grant: the mailbox needs you. Reconnect.
  await page.request.post(`${fakeZoho}/fake/expire`);
  await caption(page, "Meanwhile, the grant is withdrawn at Zoho…", 1200);
  const { connections } = await call<{ connections: { id: string; provider: string }[] }>(
    page,
    "GET",
    `/bff/api/v1/w/${workspace}/connections`,
  );
  const zoho = connections.find((connection) => connection.provider === "zoho")?.id ?? "";
  const needsYou = page.getByText("Needs you to sign in again");
  await until(async () => {
    await call(page, "POST", `/bff/api/v1/w/${workspace}/connections/${zoho}/sync`);
    return needsYou.isVisible();
  }, 90);
  await needsYou.waitFor();
  await caption(
    page,
    phone ? "…so it needs you: Reconnect" : "…so it needs you: R reconnects the chosen mailbox",
    2200,
  );
  await act(page, phone, "r", /^Reconnect /u);
  await allowAll(page, "Zoho");
  await page.getByText(/^Zoho Mail .+ is connected$/u).waitFor();
  await caption(page, "Reconnected: syncing again", 1600);

  // Both mailboxes' mail in the inbox.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(QUOTE).first().waitFor({ timeout: 120_000 });
  await page.getByText(Q4).first().waitFor({ timeout: 120_000 });
  await caption(page, "The inbox: Zoho Mail’s and Gmail’s mail", 2000);

  // Disconnect Zoho Mail, keeping its history.
  await openMailboxes(page, phone);
  const zohoRow = page.getByRole("listitem").filter({ hasText: "Zoho Mail ·" });
  if (phone) {
    await zohoRow.getByRole("button", { name: /^Disconnect/u }).click();
  } else {
    await caption(page, "J and K choose a mailbox; ⌫ disconnects it", 1400);
    await page.keyboard.press("j");
    await pause(400);
    await page.keyboard.press("Backspace");
  }
  await page.getByText(/also stops being a way to sign in/u).waitFor();
  await caption(
    page,
    "It asks what happens to the mail (delete is the default), says Zoho keeps it, and that this way in goes too",
    3400,
  );
  await page.getByRole("radio", { name: "Keep its history, read-only" }).click();
  await caption(page, "This time: keep its history, read-only", 1400);
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await page.getByText(/is disconnected$/u).waitFor();

  // Its mail reads as kept history; Zoho is no longer a way in.
  await until(async () => {
    const { inboxes } = await call<{ inboxes: { read_only: boolean }[] }>(
      page,
      "GET",
      `/bff/api/v1/w/${workspace}/inboxes`,
    );
    return inboxes.some((inbox) => inbox.read_only);
  }, 60);
  await openConversation(page, workspace, QUOTE);
  await page.getByText("Kept as history").waitFor();
  await caption(page, "Zoho’s mail: kept as history, read-only, nothing to act on", 2600);
  await until(async () => {
    const ways = await call<{ items: { provider: string }[] }>(
      page,
      "GET",
      "/bff/api/v1/identity/ways-in",
    );
    return !ways.items.some((way) => way.provider === "zoho");
  }, 60);
  await page.goto(`/w/${workspace}/settings/sign-in`);
  await page.getByRole("heading", { level: 1, name: "Ways to sign in" }).waitFor();
  await caption(page, "Ways to sign in: Zoho went with its mailbox; Google stays", 2400);

  // Disconnect Gmail, deleting its mail (the default).
  await openMailboxes(page, phone);
  await page
    .getByRole("listitem")
    .filter({ hasText: "Gmail ·" })
    .getByRole("button", { name: /^Disconnect/u })
    .click();
  await page.getByRole("radio", { name: "Delete its mail from Campbooks" }).waitFor();
  await caption(page, "Gmail: delete its mail from Campbooks (the default); Gmail keeps it", 2400);
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await page.getByText(/Its mail is leaving Campbooks/u).waitFor();
  await until(async () => {
    const { inboxes } = await call<{ inboxes: unknown[] }>(
      page,
      "GET",
      `/bff/api/v1/w/${workspace}/inboxes`,
    );
    return inboxes.length === 1;
  }, 60);
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(QUOTE).first().waitFor();
  await caption(page, "The inbox: Gmail’s mail is gone; Zoho’s kept history stays", 2800);
}
