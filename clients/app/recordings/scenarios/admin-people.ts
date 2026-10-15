import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Admin → People and Workspaces (ADR 0015, slice S2) on the real stack: a
// person signs in, an operator grants them `super` from the server's CLI,
// and they look people and workspaces up, suspend and unsuspend someone, are
// refused suspending an administrator, and check a workspace's mailboxes.
// Nothing is intercepted. Other people and workspaces are fictional demo
// rows seeded into the scratch database beforehand (Marta Silva, Silva
// Atelier, …); the server runs as in `admin-system.ts`, without `api` so its
// worker delivers events.
//
//   CAMPBOOKS_ADMIN_GRANT='CAMPBOOKS_DATABASE_URL=postgres://…/scratch \
//     target/debug/campbooks-server admin grant --account {account} --permission super --yes' \
//   pnpm --dir clients record admin-people

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

/** Signs a new person in with the fake Google and creates their first workspace. */
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
  await page.getByRole("button", { name: "Not now" }).click();
  await page.waitForURL(/\/w\/[^/]+\/?$/u);
}

/** Runs the operator's grant on the server, for `account`. */
function grantSuper(account: string): void {
  const template = process.env["CAMPBOOKS_ADMIN_GRANT"];
  if (template === undefined || template === "") {
    throw new Error("set CAMPBOOKS_ADMIN_GRANT (see the scenario's header)");
  }
  if (!/^[0-9a-f-]{36}$/u.test(account)) {
    throw new Error("the session's account id is not a UUID");
  }
  execFileSync("sh", ["-c", template.replaceAll("{account}", account)], { encoding: "utf8" });
}

/** The visible link named `name` (the sidebar's on desktop, the tab bar's on a phone). */
async function follow(page: Page, name: string): Promise<void> {
  await page.getByRole("link", { name, exact: true }).filter({ visible: true }).first().click();
}

async function scroll(page: Page, steps: number): Promise<void> {
  const viewport = page.viewportSize() ?? { width: 375, height: 812 };
  await page.mouse.move(viewport.width * 0.6, viewport.height / 2);
  for (let step = 0; step < steps; step += 1) {
    await page.mouse.wheel(0, 320);
    await pause(700);
  }
}

async function search(page: Page, label: string, text: string): Promise<void> {
  const field = page.getByRole("searchbox", { name: label });
  await field.click();
  await field.pressSequentially(text, { delay: 140 });
  await pause(900);
}

export default async function adminPeople({ page }: ScenarioContext): Promise<void> {
  await signInAsNewPerson(page);
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  grantSuper(session.user?.sub ?? "");

  // People: everyone who has signed in, newest first.
  await page.goto("/admin/people");
  await page.getByRole("heading", { level: 1, name: "People" }).waitFor();
  await page.getByRole("list", { name: "People" }).waitFor();
  await caption(page, "Admin → People: who has signed in, where they belong. Never their mail.");
  await pause(1200);
  await search(page, "Search people", "mar");
  await caption(page, "Search by the start of a name or address");
  await hideCaption(page);
  await page.getByRole("link", { name: "Open Marta Silva" }).click();

  // A person: their account and how they sign in, then Suspend.
  await page.getByRole("heading", { level: 1, name: "Marta Silva" }).waitFor();
  await page.getByText("Can sign in").waitFor();
  await caption(page, "Marta's account: ways in, live sessions, last sign-in, workspaces");
  await scroll(page, 3);
  await page.getByRole("button", { name: "Suspend…" }).click();
  await caption(page, "Suspend asks first, and says exactly what happens");
  await pause(1500);
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await page.getByText("is suspended and signed out everywhere").waitFor();
  await caption(page, "Suspended: signed out everywhere, no sign-in until unsuspended");
  await pause(1200);
  await page.getByRole("button", { name: "Unsuspend" }).click();
  await page.getByText("can sign in again").waitFor();
  await caption(page, "Unsuspend: Marta can sign in again (afresh)");
  await hideCaption(page);

  // An administrator cannot be suspended from the app.
  await page.goto("/admin/people");
  await search(page, "Search people", "sam");
  await page.getByRole("link", { name: "Open Sam Rivera" }).first().click();
  await page.getByRole("heading", { level: 1, name: "Sam Rivera" }).waitFor();
  await page.getByRole("button", { name: "Suspend…" }).click();
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await page.getByText("This person is an administrator.").waitFor();
  await caption(
    page,
    "An administrator's account is refused: revoke the profile with the CLI first",
  );
  await hideCaption(page);

  // Workspaces: members and the health of each connected mailbox.
  await follow(page, "Workspaces");
  await page.getByRole("heading", { level: 1, name: "Workspaces" }).waitFor();
  await page.getByRole("list", { name: "Workspaces" }).waitFor();
  await caption(page, "Admin → Workspaces: every workspace, its members, never its content");
  await page.getByRole("link", { name: "Open Silva Atelier" }).click();
  await page.getByRole("heading", { level: 1, name: "Silva Atelier" }).waitFor();
  await page.getByText("Needs the member to reconnect", { exact: false }).waitFor();
  await caption(page, "Members, pending invitations (counted), and each mailbox's health");
  await hideCaption(page);
  await scroll(page, 5);
  await pause(1500);
}
