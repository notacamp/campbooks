import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// A suspended member's rules pause (campbooks-cloud#147) on the real stack,
// as `inbox.ts` runs it, with the server's worker delivering the events
// (identity → workspaces → conversations):
//
//   (the fake Google, the server with `all`, and Vite as in inbox.ts)
//   CAMPBOOKS_ADMIN_GRANT='… campbooks-server admin grant --account {account} --permission super --yes' \
//   SUSPENDED_RULES_SEED='psql postgres://…/scratch -v workspace={workspace} -v inbox={inbox} -f seed.sql' \
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record suspended-rules --base-url http://localhost:5181
//
// Sam signs up and connects the demo mailbox, and teaches a rule. The seed
// command then makes Leo Marques, a fictional colleague with an account, a
// sender of Sam's inbox and the rule's author. Sam, granted `super` from the
// CLI, suspends Leo in Admin → People: Leo's rule reads as paused for now,
// without saying why (members never learn of a suspension). Lifting the
// suspension makes it act again. Fictional people and mail only.

const DIGEST = "digest@design-notes.example";

/** Runs a shell command template from the environment, filling `{name}`s. */
function run(variable: string, values: Record<string, string>): void {
  const template = process.env[variable];
  if (template === undefined || template === "") {
    throw new Error(`set ${variable} (see the scenario's header)`);
  }
  let command = template;
  for (const [name, value] of Object.entries(values)) {
    if (!/^[0-9a-f-]{36}$/u.test(value)) {
      throw new Error(`${name} is not a UUID`);
    }
    command = command.replaceAll(`{${name}}`, value);
  }
  execFileSync("sh", ["-c", command], { encoding: "utf8" });
}

async function post(page: Page, path: string, body: unknown): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: "POST",
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
    body: ${JSON.stringify(JSON.stringify(body))},
  }).then((response) => response.json())`);
}

async function rules(page: Page, workspace: string): Promise<void> {
  await page.goto(`/w/${workspace}/settings/memory`);
  await page
    .getByText(/Mail from digest@design-notes\.example/u)
    .first()
    .waitFor();
}

/** The rules again until the worker has delivered the events: `paused` or not. */
async function rulesOnce(page: Page, workspace: string, paused: boolean): Promise<void> {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    await rules(page, workspace);
    const shown = (await page.getByText(/Paused for now/u).count()) > 0;
    if (shown === paused) {
      return;
    }
    await pause(1000);
  }
  throw new Error(`the rule never read as ${paused ? "paused" : "active"}`);
}

/** Opens Leo in Admin → People. */
async function leo(page: Page): Promise<void> {
  await page.goto("/admin/people");
  await page.getByRole("heading", { level: 1, name: "People" }).waitFor();
  const field = page.getByRole("searchbox", { name: "Search people" });
  await field.click();
  await field.pressSequentially("leo", { delay: 120 });
  await page.getByRole("link", { name: "Open Leo Marques" }).first().click();
  await page.getByRole("heading", { level: 1, name: "Leo Marques" }).waitFor();
}

export default async function suspendedRules({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}`);
  const { inboxes } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/inboxes`)) as {
    inboxes: { id: string }[];
  };
  const inbox = inboxes[0]?.id ?? "";
  await post(page, `/bff/api/v1/w/${workspace}/inboxes/${inbox}/rules`, {
    conditions: [{ field: "sender_address", value: DIGEST }],
    actions: [{ action: "move_to_stream", stream: "newsletters" }],
  });
  run("SUSPENDED_RULES_SEED", { workspace, inbox });
  const session = await page.evaluate<{ user?: { sub?: string } }>(
    `fetch("/bff/session").then((response) => response.json())`,
  );
  run("CAMPBOOKS_ADMIN_GRANT", { account: session.user?.sub ?? "" });

  await rules(page, workspace);
  await caption(page, "Scout's memory: Leo taught this rule for the shared inbox", 2400);

  await leo(page);
  await caption(page, "Admin → People: suspend Leo", 1600);
  await page.getByRole("button", { name: "Suspend…" }).click();
  await pause(800);
  await page.getByRole("button", { name: "Suspend", exact: true }).click();
  await page.getByText("is suspended and signed out everywhere").waitFor();
  await caption(page, "Leo is suspended and signed out everywhere", 1600);

  await rulesOnce(page, workspace, true);
  await caption(page, "Leo's rule pauses: it acts in Leo's name. Members see no cause", 2600);

  await leo(page);
  await page.getByRole("button", { name: "Unsuspend" }).click();
  await page.getByText("can sign in again").waitFor();
  await caption(page, "Lift the suspension", 1400);

  await rulesOnce(page, workspace, false);
  await caption(page, "The rule acts again, with nothing to redo", 2600);
}
