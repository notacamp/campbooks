import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Zoho Mail on the real stack: sign in with (fake) Zoho and its mailbox
// connects, Gmail joins the same workspace, both mailboxes' mail lands in the
// inbox streams, and an archive and a reply reach Zoho. Nothing is
// intercepted: the connectors module syncs, writes back and sends through
// the fake Zoho Mail of `scripts/fake-zoho-mail.ts`, which stores what is
// sent and delivers nothing.
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
//   pnpm --dir clients record zoho-mail --base-url http://localhost:5281
//
// Fictional person and mail only; no token is ever shown.

const fakeZoho = process.env["FAKE_ZOHO_URL"] ?? "http://127.0.0.1:9172";

/** Desktop and phone differ only where the screens do: one pane or two. */
function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Call the API through the BFF from the page, as the app does. */
async function call(page: Page, method: string, path: string, body?: unknown): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)},
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
    ${body === undefined ? "" : `body: ${JSON.stringify(JSON.stringify(body))},`}
  }).then((response) => response.status === 204 ? null : response.json())`);
}

interface ZohoState {
  subject: string;
  folder: string;
  read: boolean;
  archived: boolean;
}

/** What the newest fake Zoho mailbox holds (this run's: each signs up afresh). */
async function zohoMailbox(page: Page): Promise<ZohoState[]> {
  const response = await page.request.get(`${fakeZoho}/fake/mailbox`);
  const { mailboxes } = (await response.json()) as { mailboxes: ZohoState[][] };
  return mailboxes.at(-1) ?? [];
}

/** A panel showing what Zoho holds, so the video shows the provider side. */
async function showZoho(page: Page, text: string): Promise<void> {
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-zoho") ?? document.createElement("div");
    box.id = "recording-zoho";
    box.textContent = ${JSON.stringify(`Zoho Mail (fake): ${text}`)};
    const phone = window.innerWidth < 768;
    Object.assign(box.style, {
      position: "fixed", zIndex: "2147483647", pointerEvents: "none",
      left: "8px", right: phone ? "8px" : "auto", bottom: phone ? "8px" : "16px",
      maxWidth: phone ? "none" : "560px", padding: phone ? "6px 10px" : "10px 14px",
      borderRadius: "10px", background: "rgba(200, 40, 40, 0.92)", color: "#fff",
      font: phone ? "500 12px/1.35 system-ui, sans-serif" : "500 15px/1.4 system-ui, sans-serif",
    });
    document.body.append(box);
  })()`);
}

/** Wait until the fake Zoho holds what `done` looks for. */
async function untilZoho(page: Page, done: (held: ZohoState[]) => boolean): Promise<ZohoState[]> {
  let held = await zohoMailbox(page);
  for (let attempt = 0; attempt < 120 && !done(held); attempt += 1) {
    await pause(500);
    held = await zohoMailbox(page);
  }
  return held;
}

/** Press a reader action: its key on desktop, its button on a phone. */
async function act(page: Page, phone: boolean, name: string, key: string): Promise<void> {
  await (phone
    ? page
        .getByRole("group", { name: "Conversation actions" })
        .getByRole("button", { name, exact: true })
        .click()
    : page.keyboard.press(key));
}

const QUOTE = "Orçamento para a montra de novembro";
const VAT = "IVA do 3.º trimestre: faltam dois recibos";

export default async function zohoMail({ page }: ScenarioContext): Promise<void> {
  const phone = isPhone(page);

  // Sign in with Zoho: one consent for who you are and your mail.
  await page.goto("/");
  await caption(page, "Sign in with Zoho");
  await page.getByRole("link", { name: /Continue with Zoho/u }).click();
  await page.getByText("Know who you are on Zoho").waitFor();
  await caption(page, "Zoho first asks only who you are", 1800);
  await page.getByRole("link", { name: "Allow" }).click();
  // A new account: Zoho is asked once more, for the mail.
  await page.getByText("Read, send and organise your mail").waitFor();
  await caption(page, "New here: Zoho asks once more, for your mail", 2200);
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Rivera Studio");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("heading", { name: /will be connected to this workspace/u }).waitFor();
  await caption(page, "The Zoho mailbox from sign-in connects to the workspace", 2200);
  await page.keyboard.press("Enter");
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await caption(page, "Zoho Mail connected", 1800);

  const { workspaces } = (await call(page, "GET", "/bff/api/v1/workspaces")) as {
    workspaces: { id: string }[];
  };
  const workspace = workspaces[0]?.id ?? "";

  // Gmail joins the same workspace (connectors.start_connection; the
  // consent goes through the same front door).
  await caption(page, "Connect Gmail too, in the same workspace", 1600);
  const started = (await call(
    page,
    "POST",
    `/bff/api/v1/w/${workspace}/connections/authorizations`,
    {
      provider: "google",
      capabilities: ["messages"],
    },
  )) as { authorize_url: string };
  await page.goto(started.authorize_url);
  await page.getByRole("link", { name: "Allow" }).click();
  await page.waitForURL((url) => !url.toString().includes("127.0.0.1:9171"));
  await page.goto(`/w/${workspace}`);
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await caption(page, "Two mailboxes: Zoho Mail and Gmail", 2000);

  // Both mailboxes' mail, sorted into the same streams, as it syncs.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText(QUOTE).first().waitFor({ timeout: 120_000 });
  await page.getByText("Q4 plan: can you review by Friday?").first().waitFor({ timeout: 120_000 });
  await caption(page, "Primary: Hugo and Inês from Zoho, Maya and Tiago from Gmail", 2600);
  if (!phone) {
    await page.mouse.wheel(0, 300);
    await pause(900);
    await page.mouse.wheel(0, -300);
  }

  // Archive a Zoho conversation: it reaches Zoho.
  await page.getByText(QUOTE).first().click();
  await page.getByRole("heading", { name: QUOTE }).waitFor();
  await showZoho(page, `"${QUOTE}" is in the Inbox`);
  await caption(page, phone ? "Done archives it" : "Done (E) archives it", 1800);
  await act(page, phone, "Done", "e");
  const archived = await untilZoho(
    page,
    (held) => held.find((m) => m.subject === QUOTE)?.archived === true,
  );
  const quote = archived.find((m) => m.subject === QUOTE);
  await showZoho(
    page,
    `"${QUOTE}" ${quote?.archived === true ? "is archived" : "is still in the Inbox"}`,
  );
  await caption(page, "…and Zoho archived it too", 2400);

  // Reply to Inês: it goes out through Zoho, as a reply to her message.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(VAT).first().click();
  await page.getByRole("heading", { name: VAT }).waitFor();
  await caption(page, phone ? "Reply" : "R replies", 1400);
  await (phone
    ? page.getByRole("button", { name: "Reply", exact: true }).last().click()
    : page.keyboard.press("r"));
  const composer = page.getByRole("region", { name: /^Reply to/u });
  await composer.waitFor();
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.pressSequentially("Olá Inês, envio os dois recibos hoje à tarde. Obrigado!", {
    delay: 20,
  });
  await composer.getByText("Saved").waitFor();
  await (phone
    ? composer.getByRole("button", { name: /^Send/u }).click()
    : body.press("ControlOrMeta+Enter"));
  await page.getByText(/^Sending to/u).waitFor();
  await caption(page, "It leaves after the undo window, from the Zoho mailbox", 2000);
  await page.getByText(/^Sending to/u).waitFor({ state: "hidden", timeout: 30_000 });
  const sent = await untilZoho(page, (held) =>
    held.some((m) => m.folder === "Sent" && m.subject.startsWith("Re: IVA")),
  );
  const reply = sent.find((m) => m.folder === "Sent" && m.subject.startsWith("Re: IVA"));
  await showZoho(page, reply ? `Sent holds "${reply.subject}"` : "nothing sent yet");
  // The sent message, in the thread (its body in the sandboxed frame).
  await page
    .locator('iframe[title*="Sam Rivera"]')
    .last()
    .contentFrame()
    .getByText(/envio os dois recibos/u)
    .waitFor({ timeout: 60_000 });
  await caption(page, "Sent through Zoho: the reply is in the thread, once", 2600);
}
