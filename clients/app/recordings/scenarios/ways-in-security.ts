import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Keeping an account safe (identity audit, campbooks-cloud#117), end to end
// on the real stack: the Vite dev server, the real server (API and worker)
// on a scratch database, the dev-only fake Google and Zoho, a local fake
// SMTP server that prints what it receives, and Chrome's virtual
// authenticator. Nothing is intercepted.
//
//   pnpm --dir clients fake-google &
//   pnpm --dir clients fake-zoho &
//   python3 fake_smtp.py 2525 &        # any SMTP sink on 127.0.0.1:2525
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_ZOHO__CLIENT_ID=demo-client CAMPBOOKS_ZOHO__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_ZOHO__BASE_URL=http://127.0.0.1:9052 \
//   CAMPBOOKS_MAIL__SMTP_URL=smtp://127.0.0.1:2525 \
//   CAMPBOOKS_MAIL__FROM='Campbooks <no-reply@example.com>' \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- all
//   pnpm --dir clients dev
//   pnpm --dir clients record ways-in-security
//
// Sign up with Google, add a Zoho account and a passkey (each a new way in:
// a notice goes by mail, and the page marks them New and says what to do if
// it wasn't you), then sign out everywhere. Fictional person only.

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
 * ID). The returned switch turns that off, as when nobody touches the
 * sensor (or picks the sign-in screen's autofill suggestion).
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
    .getByRole("button", { name: /^(Sam Rivera|Account)/u })
    .first()
    .click();
  await pause(700);
}

export default async function waysInSecurity({ page }: ScenarioContext): Promise<void> {
  const presence = await virtualAuthenticator(page);

  // Sign up with Google: the account and a workspace.
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).waitFor();
  await page.keyboard.press("Enter");
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.pressSequentially("Rivera Studio", { delay: 30 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("button", { name: /Not now/u }).click();
  await caption(page, "Signed up with Google. Account → Ways to sign in");

  await accountMenu(page);
  await page.getByRole("link", { name: "Ways to sign in" }).click();
  await page.getByRole("list", { name: "Your ways to sign in" }).waitFor();
  await caption(page, "One way in: the Google account she signed up with. No notice for that");

  // A new way in: a Zoho account.
  await page.getByRole("button", { name: /Add a Zoho account/u }).click();
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByText("You can now sign in with Zoho.").waitFor();
  await caption(page, "Zoho added: marked New, and the page says what to do if it wasn't you");
  await caption(page, "A notice also went by mail, to her Google and her Zoho addresses");

  // And a passkey, right after signing in (it asks for a sign-in from the
  // last five minutes).
  const add = page.getByRole("button", { name: /Add a passkey/u });
  await add.scrollIntoViewIfNeeded();
  await add.click();
  await page.getByText(/is added\. You can sign in with it now/u).waitFor();
  await caption(
    page,
    "A passkey too: New, and one more notice. It needs a sign-in from the last 5 minutes",
  );

  // Sign out everywhere.
  const everywhere = page.getByRole("button", { name: "Sign out everywhere" });
  await everywhere.scrollIntoViewIfNeeded();
  await caption(page, "If a device was lost, or a way in isn't hers: Sign out everywhere");
  await everywhere.click();
  const confirm = page.getByText(/this browser included/u);
  await confirm.waitFor();
  // Above the caption.
  await confirm.evaluate("(el) => el.scrollIntoView({ block: 'center' })");
  await caption(page, "Confirmed in place: Keep it is focused, nothing happens yet");
  // Nobody touches the sensor on the sign-in screen that follows.
  await presence(false);
  await page.getByRole("button", { name: "Sign out everywhere" }).click();
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await caption(page, "Every sign-in ended: this browser, other devices, every app. Sign in again");
}
