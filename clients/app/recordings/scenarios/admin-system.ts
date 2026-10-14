import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// The admin section (ADR 0015) on the real stack: a person signs in, has no
// Administration entry, an operator grants them `super` from the server's
// CLI, and they open Admin → System. Nothing is intercepted.
//
//   pnpm --dir clients fake-google                        # :9050
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=… CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client \
//   CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   pnpm --dir clients dev
//   CAMPBOOKS_ADMIN_GRANT='CAMPBOOKS_DATABASE_URL=postgres://…/scratch \
//     target/debug/campbooks-server admin grant --account {account} --permission super --yes' \
//   pnpm --dir clients record admin-system
//
// `CAMPBOOKS_ADMIN_GRANT` is a shell command; `{account}` is replaced by the
// signed-in account's id (the `sub` of GET /bff/session). Its output is shown
// on screen as the operator would see it (it never holds a token). Fictional
// person only.

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
      font: "500 15px/1.4 system-ui, sans-serif", whiteSpace: "pre-wrap",
      pointerEvents: "none",
    });
    document.body.append(box);
  })()`);
  await pause(1800);
}

async function hideCaption(page: Page): Promise<void> {
  await page.evaluate(`document.getElementById("recording-caption")?.remove()`);
}

/**
 * Signs a new person in with the fake Google and creates their first
 * workspace, ending on its Home. Kept in one place: the sign-in screens
 * change with the identity slices, and only this needs to follow them.
 */
async function signInAsNewPerson(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByText("Continue with Google").waitFor();
  await page.keyboard.press("1");
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Home");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  // The mailbox held from sign-up: not now (this video is about admin).
  await page.getByRole("button", { name: "Not now" }).click();
  await page.waitForURL(/\/w\/[^/]+\/?$/u);
}

/** The account menu's button: the avatar in the phone header, or the sidebar's. */
async function openAccountMenu(page: Page): Promise<void> {
  await page
    .getByRole("button", { name: /Sam Rivera|Account/u })
    .filter({ visible: true })
    .first()
    .click();
  await page.getByRole("dialog", { name: "Account" }).waitFor();
}

/** Runs the operator's grant on the server, for `account`. */
function grantSuper(account: string): string {
  const template = process.env["CAMPBOOKS_ADMIN_GRANT"];
  if (template === undefined || template === "") {
    throw new Error("set CAMPBOOKS_ADMIN_GRANT (see the scenario's header)");
  }
  if (!/^[0-9a-f-]{36}$/u.test(account)) {
    throw new Error("the session's account id is not a UUID");
  }
  return execFileSync("sh", ["-c", template.replaceAll("{account}", account)], {
    encoding: "utf8",
  }).trim();
}

export default async function adminSystem({ page }: ScenarioContext): Promise<void> {
  await signInAsNewPerson(page);
  await caption(page, "Sam Rivera is signed in: a member, nothing more");

  // A member: the account menu has no Administration entry.
  await openAccountMenu(page);
  await page.getByRole("button", { name: "Sign out" }).waitFor();
  await caption(page, "The account menu: no Administration entry for a member");
  await pause(2500);
  await page.keyboard.press("Escape");
  await hideCaption(page);

  // The operator grants super from the server's CLI (never over HTTP).
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  const output = grantSuper(session.user?.sub ?? "");
  await caption(
    page,
    `An operator grants super from the server's CLI:\n$ campbooks-server admin grant … --permission super\n${output}`,
  );
  await pause(1500);
  await hideCaption(page);

  // Now an administrator: Administration → System.
  await page.reload();
  await page.getByRole("heading", { level: 1 }).first().waitFor();
  await openAccountMenu(page);
  const entry = page.getByRole("link", { name: "Administration" });
  await entry.waitFor();
  await caption(page, "Sam is now an administrator: Administration appears in the menu");
  await entry.click();
  await page.getByRole("heading", { level: 1, name: "System" }).waitFor();
  await page.getByText("Answering").first().waitFor();
  await caption(page, "Admin → System: version, database, configuration, sign-in and AI");
  await hideCaption(page);
  await pause(1500);
  const viewport = page.viewportSize() ?? { width: 375, height: 812 };
  await page.mouse.move(viewport.width * 0.6, viewport.height / 2);
  for (let step = 0; step < 6; step += 1) {
    await page.mouse.wheel(0, 380);
    await pause(900);
  }
  await caption(page, "No secrets: addresses are shown as host only, keys and passwords never");
  await caption(
    page,
    "Each AI role says where it is: a role outside this instance's network gets no mail unless the operator allows it",
  );
  await pause(2000);
}
