import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Admin follow-ups (campbooks-cloud#122) on the real stack: Admin → People
// marks suspended accounts, the mark follows a suspend and an unsuspend, and
// Signups → Codes describes codes by their number of uses. Nothing is
// intercepted. Other people are fictional demo rows seeded into the scratch
// database beforehand (João Ferreira's account is suspended); the server
// runs as in `admin-people.ts`.
//
//   CAMPBOOKS_ADMIN_GRANT='CAMPBOOKS_DATABASE_URL=postgres://…/scratch \
//     target/debug/campbooks-server admin grant --account {account} --permission super --yes' \
//   pnpm --dir clients record admin-follow-ups

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
  await pause(2200);
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

/** The People list's row of `name`. */
function row(page: Page, name: string) {
  return page.getByRole("list", { name: "People" }).getByRole("listitem").filter({ hasText: name });
}

async function scrollTo(page: Page, text: string): Promise<void> {
  await page.getByText(text).first().scrollIntoViewIfNeeded();
  await pause(900);
}

export default async function adminFollowUps({ page }: ScenarioContext): Promise<void> {
  await signInAsNewPerson(page);
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  grantSuper(session.user?.sub ?? "");

  // People: João's account is suspended, and the list says so.
  await page.goto("/admin/people");
  await page.getByRole("heading", { level: 1, name: "People" }).waitFor();
  await row(page, "João Ferreira").getByText("Suspended").waitFor();
  await scrollTo(page, "João Ferreira");
  await caption(page, "Admin → People now marks suspended accounts (João)");

  // Suspend Marta from her page; back on the list, she is marked too.
  await hideCaption(page);
  await page.getByRole("link", { name: "Open Marta Silva" }).click();
  await page.getByRole("heading", { level: 1, name: "Marta Silva" }).waitFor();
  await page.getByRole("button", { name: "Suspend…" }).click();
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await page.getByText("is suspended and signed out everywhere").waitFor();
  await caption(page, "Suspend Marta…");
  await page.getByRole("link", { name: "All people" }).first().click();
  await row(page, "Marta Silva").getByText("Suspended").waitFor();
  await scrollTo(page, "Marta Silva");
  await caption(page, "…and the list marks her at once");

  // Unsuspend: the mark goes away.
  await hideCaption(page);
  await page.getByRole("link", { name: "Open Marta Silva" }).click();
  await page.getByRole("button", { name: "Unsuspend" }).click();
  await page.getByText("can sign in again").waitFor();
  await page.getByRole("link", { name: "All people" }).first().click();
  await row(page, "Marta Silva").waitFor();
  await page.waitForFunction(
    `![...document.querySelectorAll("li")].some((li) => li.textContent.includes("Marta Silva") && li.textContent.includes("Suspended"))`,
  );
  await scrollTo(page, "Marta Silva");
  await caption(page, "Unsuspended: no mark");
  await hideCaption(page);

  // Signups → Codes: a code is good for the number of uses you set.
  await page.goto("/admin/signups");
  await page.getByRole("heading", { level: 1 }).first().waitFor();
  await page.getByRole("button", { name: "Codes" }).click();
  await page.getByText("Each code works for as many signups as you set").waitFor();
  await caption(page, "Signups → Codes: no more “single-use… as many times as you set”");
  await pause(1500);
}
