import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Admin → Jobs (ADR 0015) on the real stack: an administrator opens the job
// queue, reads why jobs gave up, copies an error, retries one, discards one
// after a confirmation and retries a whole kind. Nothing is intercepted.
//
// Set up exactly like `admin-system` (fake Google, the server in `api` mode
// on a scratch database, `pnpm --dir clients dev`), plus:
//
//   CAMPBOOKS_ADMIN_GRANT='… admin grant --account {account} --permission super --yes'
//   CAMPBOOKS_JOBS_SEED='psql "$SCRATCH_URL" -q -f seed-jobs.sql'
//   pnpm --dir clients record admin-jobs
//
// `CAMPBOOKS_JOBS_SEED` is a shell command that puts fictional jobs in the
// queue (failed ones with made-up errors); it runs as each recording starts.
// The server runs without a worker, so a retried job simply waits, ready.
// Fictional person and data only.

/** A caption pinned to the bottom of the page, so the video explains itself. */
async function caption(page: Page, text: string): Promise<void> {
  // A script string: the recordings are type-checked without the DOM types.
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-caption") ?? document.createElement("div");
    box.id = "recording-caption";
    box.textContent = ${JSON.stringify(text)};
    Object.assign(box.style, {
      position: "fixed", left: "12px", right: "12px",
      bottom: window.innerWidth < 500 ? "76px" : "12px",
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

/** Runs a shell command from the environment (`{account}` is replaced). */
function run(variable: string, account?: string): string {
  const template = process.env[variable];
  if (template === undefined || template === "") {
    throw new Error(`set ${variable} (see the scenario's header)`);
  }
  if (account !== undefined && !/^[0-9a-f-]{36}$/u.test(account)) {
    throw new Error("the session's account id is not a UUID");
  }
  return execFileSync("sh", ["-c", template.replaceAll("{account}", account ?? "")], {
    encoding: "utf8",
  }).trim();
}

export default async function adminJobs({ page }: ScenarioContext): Promise<void> {
  await signInAsNewPerson(page);
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  run("CAMPBOOKS_ADMIN_GRANT", session.user?.sub ?? "");
  run("CAMPBOOKS_JOBS_SEED");

  await page.goto("/admin/jobs");
  await page.getByRole("heading", { level: 1, name: "Jobs" }).waitFor();
  const queue = page.getByRole("list", { name: "Queue" });
  await queue.waitFor();
  await caption(page, "Admin → Jobs: the queue by kind — waiting, running, failed");
  await pause(1500);

  const failed = page.getByRole("list", { name: "Failed jobs" });
  await failed.scrollIntoViewIfNeeded();
  await caption(page, "Jobs that gave up, with the last error — never what they carried");
  await failed.getByRole("button", { name: "Copy error" }).first().click();
  await caption(page, "Each error is monospace, wrapped, and one click to copy");
  await hideCaption(page);

  // Retry one.
  await failed.getByRole("button", { name: "Retry" }).first().click();
  await page.getByText("The job is back in the queue.").waitFor();
  await caption(page, "Retry puts the job back on the queue; the counts update");
  await hideCaption(page);

  // Discard asks first.
  await failed.getByRole("button", { name: "Discard" }).first().click();
  await page.getByText(/It is deleted for good/u).waitFor();
  await caption(page, "Discard asks first: it deletes the job for good");
  await pause(1200);
  await page.getByRole("group").getByRole("button", { name: "Discard" }).click();
  await page.getByText("The job was discarded.").waitFor();
  await hideCaption(page);

  // Retry all of a kind.
  await queue.scrollIntoViewIfNeeded();
  await queue.getByRole("button", { name: "Retry all failed" }).first().click();
  await caption(page, "Retry all of a kind names how many, and asks first");
  await page.getByRole("group").getByRole("button", { name: "Retry" }).click();
  await page
    .getByText(/are back in the queue|is back in the queue/u)
    .first()
    .waitFor();
  await caption(page, "Done: the failed list is empty and the jobs wait for a worker");
  await pause(1500);
}
