import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// An instance that lets in only its owner's people (`CAMPBOOKS_SIGNUP__ALLOWED`),
// on the real stack: the Vite dev server, the real server on a scratch
// database and the fake Google from `scripts/fake-provider.ts`. The fake
// Google's one person is not on the list. Set up as in `sign-in-built-in.ts`,
// plus `CAMPBOOKS_SIGNUP__ALLOWED="owner@example.com, example.org"`,
// `CAMPBOOKS_BLOBS__PATH` and `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` for a
// scratch database.
//
//   pnpm --dir clients record sign-in-allow-list
//
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

export default async function signInAllowList({ page }: ScenarioContext): Promise<void> {
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).waitFor();
  await caption(page, "This instance lets in only the owner's people");
  await page.keyboard.press("1");
  const allow = page.getByRole("link", { name: "Allow" });
  await allow.waitFor();
  await caption(page, "A Google account whose address is not on the list");
  await allow.click();

  // Back on the sign-in screen, with nothing created or kept.
  const alert = page.getByText("This Campbooks isn’t open to this address.");
  await alert.waitFor();
  await caption(page, "Refused, and nothing was kept: no account, no session");
  await pause(2500);
}
