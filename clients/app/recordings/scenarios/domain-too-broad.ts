import { execFileSync } from "node:child_process";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// A stored rule naming a public suffix stops matching (campbooks-cloud#163),
// on the real stack, as `inbox.ts` runs it (the fake Google, the server with
// `all`, and Vite):
//
//   DOMAIN_TOO_BROAD_SEED='psql postgres://…/scratch -v workspace={workspace} -f seed.sql' \
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record domain-too-broad --base-url http://localhost:5181
//
// Sam teaches a rule for a sender domain. The seed command then rewrites its
// domain to `github.io`, as a rule stored before the Public Suffix List was
// complete would read: the rule reads as paused, says why, and can be
// removed. Fictional people and mail only.

const DOMAIN = "design-notes.example";

/** Runs the seed command from the environment, filling `{workspace}`. */
function seed(workspace: string): void {
  const template = process.env["DOMAIN_TOO_BROAD_SEED"];
  if (template === undefined || template === "") {
    throw new Error("set DOMAIN_TOO_BROAD_SEED (see the scenario's header)");
  }
  if (!/^[0-9a-f-]{36}$/u.test(workspace)) {
    throw new Error("workspace is not a UUID");
  }
  execFileSync("sh", ["-c", template.replaceAll("{workspace}", workspace)], { encoding: "utf8" });
}

async function post(page: Page, path: string, body: unknown): Promise<unknown> {
  return page.evaluate(`fetch(${JSON.stringify(path)}, {
    method: "POST",
    headers: { "x-campbooks-csrf": "1", "content-type": "application/json" },
    body: ${JSON.stringify(JSON.stringify(body))},
  }).then((response) => response.json())`);
}

export default async function domainTooBroad({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}`);
  const { inboxes } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/inboxes`)) as {
    inboxes: { id: string }[];
  };
  const inbox = inboxes[0]?.id ?? "";
  await post(page, `/bff/api/v1/w/${workspace}/inboxes/${inbox}/rules`, {
    conditions: [{ field: "sender_domain", value: DOMAIN }],
    actions: [{ action: "move_to_stream", stream: "newsletters" }],
  });
  await page.goto(`/w/${workspace}/settings/memory`);
  await page.getByText(DOMAIN).first().waitFor();
  await caption(page, "Scout's memory: a rule for one sender domain", 2400);

  seed(workspace);
  await page.goto(`/w/${workspace}/settings/memory`);
  await page.getByText(/^Paused: its domain is too broad/u).waitFor();
  await caption(page, "Stored before, its domain is now known to cover a whole group of sites", 2800);
  await caption(page, "It matches nothing and says why. Remove is still there", 2400);

  await page.getByRole("button", { name: "Remove" }).first().click();
  await pause(800);
  await caption(page, "Removing it is the member's choice", 1600);
}
