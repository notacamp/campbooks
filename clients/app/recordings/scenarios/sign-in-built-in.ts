import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Signing up and in through Campbooks' own identity provider (ADR 0014,
// slice S1a), end to end on the real stack: the Vite dev server, the real
// server and BFF on a scratch database, and the fake Google from
// `recordings/fake-google.ts`. Nothing is intercepted.
//
//   pnpm --dir clients fake-google                        # :9050
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   pnpm --dir clients dev
//   pnpm --dir clients record sign-in-built-in
//
// The sign-in and new-account screens are slice S1b: until they land, this
// scenario does their two steps itself (choose Google; confirm the new
// account), and says so on screen. Fictional person only; no token is shown.

/** A caption pinned to the bottom of the page, so the video explains itself. */
async function caption(page: Page, text: string): Promise<void> {
  // A script string: the recordings are type-checked without the DOM types.
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-caption") ?? document.createElement("div");
    box.id = "recording-caption";
    box.textContent = ${JSON.stringify(text)};
    Object.assign(box.style, {
      position: "fixed", left: "12px", right: "12px", bottom: "12px",
      zIndex: "2147483647", padding: "10px 14px", borderRadius: "10px",
      background: "rgba(20, 20, 24, 0.88)", color: "#fff",
      font: "500 15px/1.4 system-ui, sans-serif",
    });
    document.body.append(box);
  })()`);
  await pause(1600);
}

/** The `request` the issuer handed the sign-in screen. */
async function signInRequest(page: Page): Promise<string> {
  await page.waitForURL(/\/sign-in\?request=/u);
  return new URL(page.url()).searchParams.get("request") ?? "";
}

/** What S1b's sign-in screen will do: start a round trip to Google. */
async function chooseGoogle(page: Page, request: string): Promise<void> {
  await caption(page, "Campbooks' issuer asks how to sign in (S1b's screen goes here): Google");
  await page.goto(`/auth/start/google?request=${encodeURIComponent(request)}`);
  await page.getByRole("heading", { name: "Sign in to Campbooks" }).waitFor();
  await caption(page, "One Google consent: who you are, and your mail, with offline access");
  await page.getByRole("link", { name: "Allow" }).click();
}

/** Call the API through the BFF from the page, as the app does. */
async function api(page: Page, method: string, path: string): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)},
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
  }).then((response) => response.json())`);
}

/** Show a JSON answer of the API on screen. */
async function showJson(page: Page, path: string, text: string): Promise<void> {
  await page.goto(path);
  await page.addStyleTag({ content: "body,pre{font-size:15px!important;white-space:pre-wrap}" });
  // A raw JSON page has no viewport: phones would show it zoomed out.
  await page.evaluate(`(() => {
    const meta = document.createElement("meta");
    meta.name = "viewport";
    meta.content = "width=device-width, initial-scale=1";
    document.head.append(meta);
    const pre = document.querySelector("pre");
    if (pre?.textContent) pre.textContent = JSON.stringify(JSON.parse(pre.textContent), null, 2);
  })()`);
  await caption(page, text);
  await pause(1200);
}

export default async function signInBuiltIn({ page }: ScenarioContext): Promise<void> {
  await page.goto("/");
  await page.getByRole("link", { name: "Sign in" }).waitFor();
  await caption(
    page,
    "Signed out. Sign in goes to the web app's BFF, then to Campbooks' own issuer",
  );
  await page.getByRole("link", { name: "Sign in" }).click();

  await chooseGoogle(page, await signInRequest(page));

  // A Google account no Campbooks account uses yet: S1b's "Create account".
  await page.waitForURL(/\/sign-in\/new/u);
  await caption(page, "New here: nothing exists until you confirm (S1b's screen) — Create account");
  const created = (await api(page, "POST", "/auth/new-account")) as { redirect: string };
  await page.goto(created.redirect);

  // Back in the app, signed in: the first run asks for a workspace.
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await caption(page, "Signed in as Sam Rivera. First run: create a workspace");
  await name.pressSequentially("Home", { delay: 60 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("heading", { name: "Home is ready" }).waitFor();
  await pause(800);

  const { workspaces } = (await api(page, "GET", "/bff/api/v1/workspaces")) as {
    workspaces: { id: string }[];
  };
  const workspace = workspaces[0]?.id ?? "";
  await showJson(
    page,
    "/bff/api/v1/identity/held-grants",
    "The mailbox granted at sign-up waits for a workspace (never its tokens)",
  );
  const held = (await api(page, "GET", "/bff/api/v1/identity/held-grants")) as {
    items: { id: string }[];
  };
  await api(
    page,
    "POST",
    `/bff/api/v1/w/${workspace}/identity/held-grants/${held.items[0]?.id ?? ""}/adopt`,
  );
  await showJson(
    page,
    `/bff/api/v1/w/${workspace}/connections`,
    "Adopted (S1b's first-run step): Home's Gmail connection, with no second consent",
  );

  // Sign out, then in again: the same account and its workspace.
  await page.goto(`/w/${workspace}`);
  await page.getByRole("heading", { name: "Nothing needs you yet" }).waitFor();
  await page
    .getByRole("button", { name: /^(Sam Rivera|Account)/u })
    .first()
    .click();
  await caption(page, "Sign out ends the web session and the issuer's, no confirmation page");
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByRole("link", { name: "Sign in" }).waitFor();
  await caption(page, "Signed out. Sign in again with the same Google account");
  await page.getByRole("link", { name: "Sign in" }).click();
  await chooseGoogle(page, await signInRequest(page));
  await page.getByRole("heading", { name: "Nothing needs you yet" }).waitFor();
  await caption(page, "Back in the same account, in Home: no new account, no new grant");
  await pause(1500);
}
