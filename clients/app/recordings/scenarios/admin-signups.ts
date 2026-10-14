import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Admin → Signups (ADR 0015, slice S1) on the real stack, nothing intercepted:
// an instance that approves new accounts. The first person bootstraps it and is
// made an administrator from the server's CLI; a second person asks to join and
// waits; the administrator approves them and mints a single-use code; the second
// person comes back approved and creates their workspace.
//
//   pnpm --dir clients fake-google --port 9097
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5197 \
//   CAMPBOOKS_BIND_ADDR=127.0.0.1:18097 CAMPBOOKS_SIGNUP__POLICY=approval \
//   CAMPBOOKS_SECRET_KEY=… CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client \
//   CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9097 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   CAMPBOOKS_API_URL=http://127.0.0.1:18097 pnpm --dir clients dev --port 5197
//   CAMPBOOKS_ADMIN_GRANT='CAMPBOOKS_DATABASE_URL=postgres://…/scratch \
//     target/debug/campbooks-server admin grant --account {account} --permission super --yes' \
//   CAMPBOOKS_DEMO_RESET='psql postgres://…/scratch -c "TRUNCATE workspaces.users, workspaces.workspaces, workspaces.signup_codes CASCADE"' \
//   pnpm --dir clients record admin-signups --base-url http://localhost:5197
//
// Everyone who signs in is the fictional Sam Rivera; each browser state is a
// different account of hers. Fictional people only.

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

/** Signs in through the fake Google as whoever this browser state is. */
async function signIn(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByText("Continue with Google").waitFor();
  await page.keyboard.press("1");
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).click();
}

/** Answers the two questions of the create-workspace flow, skipping neither. */
async function nameWorkspace(page: Page, name: string): Promise<void> {
  const field = page.getByRole("textbox", { name: "Workspace name" });
  await field.waitFor();
  await field.fill(name);
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
}

/**
 * Starts from an instance nobody has signed in to: the recorder runs this
 * scenario twice (375px, desktop) against one server, and the first person to
 * arrive must be the instance's first user again. `CAMPBOOKS_DEMO_RESET` is a
 * shell command that empties the demo database's workspaces tables.
 */
function resetInstance(): void {
  const command = process.env["CAMPBOOKS_DEMO_RESET"];
  if (command === undefined || command === "") {
    throw new Error("set CAMPBOOKS_DEMO_RESET (see the scenario's header)");
  }
  execFileSync("sh", ["-c", command], { encoding: "utf8" });
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

export default async function adminSignups({ page }: ScenarioContext): Promise<void> {
  const context = page.context();
  resetInstance();

  // The first person bootstraps the instance and becomes its administrator.
  await signIn(page);
  await nameWorkspace(page, "Home");
  await page.getByRole("button", { name: "Not now" }).click();
  await page.waitForURL(/\/w\/[^/]+\/?$/u);
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  grantSuper(session.user?.sub ?? "");
  const administrator = await context.storageState();

  // A second person asks to join and waits.
  await context.clearCookies();
  await signIn(page);
  await nameWorkspace(page, "Barros & Co");
  await page.getByRole("heading", { name: "Your request is waiting" }).waitFor();
  await caption(page, "A new person asks to join an instance that approves signups: waiting");
  await hideCaption(page);
  const requester = await context.storageState();

  // The administrator sees the request and approves it.
  await context.clearCookies();
  await context.addCookies(administrator.cookies);
  await page.goto("/admin/signups");
  await page.getByRole("heading", { level: 1, name: "Signups" }).waitFor();
  await page
    .getByRole("button", { name: /^Approve / })
    .first()
    .waitFor();
  await caption(page, "Admin → Signups: the request is waiting for an administrator");
  await page
    .getByRole("button", { name: /^Approve / })
    .first()
    .click();
  await caption(page, "Approving asks first, in place");
  await page
    .getByRole("group", { name: /^Approve .*\?/u })
    .getByRole("button", { name: "Approve", exact: true })
    .click();
  await page.getByRole("listitem").getByText("Approved", { exact: true }).waitFor();
  await caption(page, "Approved. Decisions are idempotent and written to the security log");
  await hideCaption(page);

  // And mints a single-use code, shown once.
  await page.getByRole("button", { name: "Codes" }).click();
  await page.getByRole("textbox", { name: "Label" }).fill("Beta, Inês");
  await page.getByRole("button", { name: "Mint code" }).click();
  await page.getByRole("textbox", { name: "Signup code" }).waitFor();
  await caption(page, "A code is shown once: only its digest is stored, so it can't be read later");
  await hideCaption(page);
  await page.getByRole("button", { name: "I’ve copied it" }).click();
  await page.getByText("Beta, Inês").first().waitFor();
  await caption(
    page,
    "The list shows uses, expiry and state, never the code. Revoke is one click away",
  );
  await hideCaption(page);

  // The second person comes back approved and creates their workspace.
  await context.clearCookies();
  await context.addCookies(requester.cookies);
  await page.goto("/");
  await page.getByRole("heading", { name: "You’re approved" }).waitFor();
  await caption(page, "Next time the second person opens the app: approved");
  await hideCaption(page);
  await page.getByRole("button", { name: "Continue" }).click();
  await nameWorkspace(page, "Barros & Co");
  // The mailbox held since sign-up: not now (this video is about signups).
  await page.getByRole("button", { name: "Not now" }).click();
  await page.waitForURL(/\/w\/[^/]+\/?$/u);
  await caption(page, "…and their workspace is created");
  await pause(1200);
}
