import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Signing up and in through Campbooks' own identity provider (ADR 0014,
// slice S1a), end to end on the real stack: the Vite dev server, the real
// server and BFF on a scratch database, and the fake Google from
// `scripts/fake-provider.ts`. Nothing is intercepted.
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
// Every screen is the real one: the sign-in and new-account screens (S1b),
// the first run with its mailbox step, Home, and signing out and in again.
// Fictional person only; no token is shown.

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
      pointerEvents: "none",
    });
    document.body.append(box);
  })()`);
  await pause(1600);
}

/** On the sign-in screen: Google is `1`; the fake Google's consent: Allow. */
async function continueWithGoogle(page: Page): Promise<void> {
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await pause(1200);
  await page.keyboard.press("1");
  const allow = page.getByRole("link", { name: "Allow" });
  await allow.waitFor();
  await caption(page, "One Google consent: who you are, and your mail, with offline access");
  await allow.click();
}

export default async function signInBuiltIn({ page }: ScenarioContext): Promise<void> {
  // Signed out: the app goes through the BFF to Campbooks' own issuer, which
  // shows its sign-in screen.
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await caption(
    page,
    "Signed out: Campbooks' own sign-in screen, before anything leaves for Google",
  );
  await page.getByText("What Campbooks reads and keeps").click();
  await pause(2500);
  await continueWithGoogle(page);

  // A Google account no Campbooks account uses yet.
  const create = page.getByRole("button", { name: /Create account/u });
  await create.waitFor();
  await caption(page, "New here: nothing exists until you confirm. Enter creates the account");
  await pause(1200);
  await page.keyboard.press("Enter");

  // Signed in: the first run asks for a workspace.
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await pause(800);
  await name.pressSequentially("Home", { delay: 60 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).click();
  await pause(600);
  await page.getByRole("button", { name: "Create workspace" }).click();

  // The mailbox granted at sign-up: connect it here, with no second consent.
  await page
    .getByRole("heading", {
      name: "Your Gmail sam.rivera@example.com will be connected to this workspace",
    })
    .waitFor();
  await caption(page, "The Gmail from sign-up, held for 7 days: Enter connects it here");
  await pause(1500);
  await page.keyboard.press("Enter");

  // Home names the connected mailbox.
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await caption(page, "Home: the mailbox is connected");
  await pause(1500);

  // Sign out, then in again: the same account and its workspace.
  await page
    .getByRole("button", { name: /^(Sam Rivera|Account)/u })
    .first()
    .click();
  await pause(800);
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await caption(page, "Signed out. Sign in again with the same Google account");
  await continueWithGoogle(page);
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await caption(page, "Back in the same account and workspace: no new account, no new grant");
  await pause(2000);
}
