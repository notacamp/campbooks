import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// More ways in (ADR 0014, slice S2), end to end on the real stack: the Vite
// dev server, the real server and BFF on a scratch database, and the dev-only
// fake providers from `scripts/fake-provider.ts`. Nothing is intercepted.
//
//   pnpm --dir clients fake-google & pnpm --dir clients fake-microsoft & pnpm --dir clients fake-zoho
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_MICROSOFT__CLIENT_ID=demo-client CAMPBOOKS_MICROSOFT__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_MICROSOFT__BASE_URL=http://127.0.0.1:9051 \
//   CAMPBOOKS_ZOHO__CLIENT_ID=demo-client CAMPBOOKS_ZOHO__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_ZOHO__BASE_URL=http://127.0.0.1:9052 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   pnpm --dir clients dev
//   pnpm --dir clients record ways-in
//
// Sign up with Google, add Zoho as a way in, sign out, sign in with Zoho into
// the same account, then remove Google; removing Zoho, the last way in, is
// refused. Fictional person only; no token is shown.

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

/** Allow on a fake provider's consent page. */
async function allow(page: Page, provider: string): Promise<void> {
  await page.getByText(`Fake ${provider} · development only`).waitFor();
  await caption(page, `${provider}'s consent (a dev-only fake): Allow`);
  await page.getByRole("link", { name: "Allow" }).click();
}

/** Open the account menu (the avatar on a phone, the footer on desktop). */
async function accountMenu(page: Page): Promise<void> {
  await page
    .getByRole("button", { name: /^(Sam Rivera|Account)/u })
    .first()
    .click();
  await pause(700);
}

async function openWaysIn(page: Page): Promise<void> {
  await accountMenu(page);
  await page.getByRole("link", { name: "Ways to sign in" }).click();
  await page.getByRole("heading", { level: 1, name: "Ways to sign in" }).waitFor();
}

export default async function waysIn({ page }: ScenarioContext): Promise<void> {
  // Sign up with Google: the account, a workspace, its mailbox.
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await caption(page, "Signed out: Google, Microsoft and Zoho are all on offer");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await allow(page, "Google");
  await page.getByRole("button", { name: /Create account/u }).waitFor();
  await pause(800);
  await page.keyboard.press("Enter");
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.pressSequentially("Duarte & Pessoa", { delay: 40 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await pause(400);
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("button", { name: /Not now/u }).click();
  await caption(page, "Signed up with Google. Now: Account → Ways to sign in");

  // Ways to sign in: Google, the one signed up with. Add Zoho.
  await openWaysIn(page);
  await caption(page, "One way in so far: the Google account this person signed up with");
  await page.keyboard.press("a");
  await pause(600);
  await page.getByRole("button", { name: /Add a Zoho account/u }).click();
  await allow(page, "Zoho");
  await page.getByText("You can now sign in with Zoho.").waitFor();
  await caption(page, "Back, linked by the completed round trip (never by email): two ways in");
  await pause(1200);

  // Sign out, then sign in with Zoho: the same account.
  await accountMenu(page);
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByRole("link", { name: /Continue with Zoho/u }).waitFor();
  await caption(page, "Signed out. Sign in with Zoho this time (key 3)");
  await page.keyboard.press("3");
  await allow(page, "Zoho");
  await page.getByRole("link", { name: "Home" }).first().waitFor();
  await caption(page, "Zoho opens the same account and its workspace, Duarte & Pessoa");

  // Remove Google; Zoho, now the only way in, can't go.
  await openWaysIn(page);
  await page.getByRole("button", { name: /^Remove Google/u }).click();
  await caption(page, "Remove asks first, in place. The mailbox stays connected");
  await page.getByRole("button", { name: "Remove", exact: true }).click();
  await page.getByRole("button", { name: /^Remove Google/u }).waitFor({ state: "detached" });
  await caption(page, "Google is gone: Zoho is the only way in left");
  await page.getByRole("button", { name: /^Remove Zoho/u }).click();
  await pause(800);
  await page.getByRole("button", { name: "Remove", exact: true }).click();
  await page.getByText("This is your only way to sign in.").waitFor();
  await caption(page, "The last way in is refused (last_way_in): add another first");
  await pause(1500);
}
