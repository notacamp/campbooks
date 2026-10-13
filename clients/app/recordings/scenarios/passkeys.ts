import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Passkeys (ADR 0014, slice S3), end to end on the real stack: the Vite dev
// server, the real server and BFF on a scratch database, a dev-only fake
// Google (so the sign-in screen looks as it does in production), and
// Chrome's virtual authenticator (WebAuthn over CDP) standing in for Touch
// ID. Nothing is intercepted: every ceremony is verified by the server.
//
//   pnpm --dir clients fake-google &
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   pnpm --dir clients dev
//   pnpm --dir clients record passkeys
//
// Create an account with a passkey alone, make a first workspace, find the
// passkey under Ways to sign in (rename it; removing the only way in is
// refused; this device can't add a second one for the same account), sign
// out, and sign back in with `P`.
// Fictional person only; no credential or token is shown.

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
      font: "500 15px/1.4 system-ui, sans-serif", pointerEvents: "none",
    });
    document.body.append(box);
  })()`);
  await pause(1800);
}

/**
 * A platform authenticator that verifies the user at once (as after Touch
 * ID). The returned switch turns that on and off: off, nothing answers, as
 * when nobody touches the sensor (or picks the autofill suggestion).
 */
async function virtualAuthenticator(page: Page): Promise<(on: boolean) => Promise<void>> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  const { authenticatorId } = await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  return async (on) => {
    await cdp.send("WebAuthn.setAutomaticPresenceSimulation", { authenticatorId, enabled: on });
  };
}

/** Open the account menu (the avatar on a phone, the footer on desktop). */
async function accountMenu(page: Page): Promise<void> {
  await page
    .getByRole("button", { name: /^(Ana Lima|Account)/u })
    .first()
    .click();
  await pause(700);
}

export default async function passkeys({ page }: ScenarioContext): Promise<void> {
  const presence = await virtualAuthenticator(page);

  // Signed out: Google, or a passkey; and a passkey-only sign-up.
  await page.goto("/");
  await page.getByRole("button", { name: /Use a passkey/u }).waitFor();
  await caption(page, "Signed out: Google, or a passkey (P). No account yet: create one");
  await page.getByRole("link", { name: /Create an account with a passkey/u }).click();

  // One question, then the device makes the passkey.
  const name = page.getByRole("textbox", { name: "What should we call you?" });
  await name.waitFor();
  await caption(page, "A new account with a passkey alone: no provider, no password");
  await name.pressSequentially("Ana Lima", { delay: 50 });
  await page.keyboard.press("Enter");
  await caption(page, "The device saves a passkey (here Chrome's virtual authenticator)");

  // First workspace.
  const workspace = page.getByRole("textbox", { name: "Workspace name" });
  await workspace.waitFor();
  await workspace.pressSequentially("Lima Studio", { delay: 40 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await pause(400);
  await page.getByRole("button", { name: "Create workspace" }).click();
  // No mailbox came with this sign-up: the workspace is simply ready.
  await page.getByRole("button", { name: "Open Lima Studio" }).click();
  await page.getByRole("link", { name: "Home" }).first().waitFor();
  await caption(page, "Signed in with the passkey. Now: Account → Ways to sign in");

  // Ways to sign in → Passkeys.
  await accountMenu(page);
  await page.getByRole("link", { name: "Ways to sign in" }).click();
  const passkeysList = page.getByRole("list", { name: "Your passkeys" });
  await passkeysList.waitFor();
  await passkeysList.scrollIntoViewIfNeeded();
  await caption(page, "The passkey is this account's only way in");

  await page.getByRole("button", { name: "Rename Passkey" }).click();
  const field = page.getByRole("textbox", { name: "Name" });
  await field.fill("");
  await field.pressSequentially("Studio laptop", { delay: 40 });
  await page.keyboard.press("Enter");
  await page.getByText("Studio laptop").waitFor();
  await caption(page, "Renamed in place");

  await page.getByRole("button", { name: "Remove Studio laptop" }).click();
  await pause(800);
  await page.getByRole("button", { name: "Remove", exact: true }).click();
  await page.getByText("This is your only way to sign in.").waitFor();
  await caption(page, "Removing the last way in is refused (last_way_in)");

  // N then P adds a passkey; this device already holds one for the account.
  await page.keyboard.press("n");
  await page.keyboard.press("p");
  await page.getByText("This device already has a passkey for your account.").waitFor();
  await caption(page, "N P adds a passkey; this device already holds one (excludeCredentials)");

  // Sign out, then back in with P. (Nobody picks the autofill suggestion
  // meanwhile: the sensor is untouched until P.)
  await presence(false);
  await accountMenu(page);
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByRole("button", { name: /Use a passkey/u }).waitFor();
  await caption(page, "Signed out. Saved passkeys are offered in autofill, or press P");
  await presence(true);
  await page.keyboard.press("p");
  await page.getByRole("link", { name: "Home" }).first().waitFor();
  await caption(page, "The passkey opens the same account and its workspace, Lima Studio");
  await pause(1500);
}
