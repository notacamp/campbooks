import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// The decide role's answers check (campbooks-cloud#128) on the real server:
// at startup the server asks the decide model about eight fictional emails
// with obvious answers. Until that check finishes the model is not used (the
// System panel says it is being checked, not that it is healthy); once it
// fails, the panel says its answers are not used. The decide endpoint is a
// local stand-in replaying the answers laya-serve (the `deploy/laya` image)
// gave for those emails, holding them until `CAMPBOOKS_FAKE_LAYA_RELEASE`
// exists, so the pending state can be seen. The 375px run shows it pending;
// the desktop run releases the answers and shows the check failing.
//
//   python3 fake_laya.py 18000 /tmp/release        # the stand-in
//   CAMPBOOKS_AI__DECIDE__PROTOCOL=laya CAMPBOOKS_AI__DECIDE__BASE_URL=http://127.0.0.1:18000/v1 \
//   CAMPBOOKS_AI__DECIDE__MODEL=convaiinnovations/laya-multilingual \
//   cargo run -p campbooks-server -- all            # plus the settings of `admin-system.ts`
//   CAMPBOOKS_FAKE_LAYA_RELEASE=/tmp/release \
//   CAMPBOOKS_ADMIN_GRANT='…/campbooks-server admin grant --account {account} --permission super --yes' \
//   pnpm --dir clients record admin-decide-check
//
// Fictional person only.

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

/** Lets the stand-in answer the check (see the header). */
function releaseAnswers(): void {
  const release = process.env["CAMPBOOKS_FAKE_LAYA_RELEASE"];
  if (release === undefined || release === "") {
    throw new Error("set CAMPBOOKS_FAKE_LAYA_RELEASE (see the scenario's header)");
  }
  writeFileSync(release, "");
}

export default async function adminDecideCheck({ page }: ScenarioContext): Promise<void> {
  await signInAsNewPerson(page);
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  grantSuper(session.user?.sub ?? "");
  await caption(page, "Sam Rivera runs this instance and is an administrator");
  await hideCaption(page);

  await page.reload();
  await page.getByRole("heading", { level: 1 }).first().waitFor();
  await openAccountMenu(page);
  await page.getByRole("link", { name: "Administration" }).click();
  await page.getByRole("heading", { level: 1, name: "System" }).waitFor();

  const narrow = (page.viewportSize()?.width ?? 1440) < 600;
  const checking = page.getByText("Checking its answers").first();
  await checking.waitFor();
  await checking.scrollIntoViewIfNeeded();
  await pause(1200);
  await caption(
    page,
    "Right after startup the server is still checking the decisions model on eight made-up emails with obvious answers",
  );
  await caption(page, "Until it passes, its answers are not used: the rules sort mail");
  if (narrow) {
    await hideCaption(page);
    await pause(2000);
    return;
  }

  releaseAnswers();
  await hideCaption(page);
  const failed = page.getByText("Answers fail the check").first();
  for (let attempt = 0; attempt < 20; attempt += 1) {
    await page.reload();
    await page.getByRole("heading", { level: 1, name: "System" }).waitFor();
    if (await failed.isVisible()) {
      break;
    }
    await pause(1000);
  }
  await failed.scrollIntoViewIfNeeded();
  await pause(1200);
  await caption(
    page,
    "Laya got only 8 of 33 right (it says yes to almost everything), so its answers are not used",
  );
  await caption(
    page,
    "It is checked again every 15 minutes while it fails, and every 6 hours once it passes",
  );
  await hideCaption(page);
  await pause(2500);
}
