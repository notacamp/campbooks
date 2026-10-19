import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { api, caption, signedIn } from "./inbox.ts";

// Trusted-sender images follow a removed rule live (campbooks-cloud#236), on
// the real stack started as inbox.ts says, with a decide model as
// trusted-images.ts needs. The address choice says what the checks vouch
// for (its domain); then the rule is removed from outside this screen (as
// another device or member would) and the open message holds its images back
// again with no reload. Trusted again and left, the rule removed once more:
// opened again, the newsletter waits for a fresh read (its skeleton), never
// its kept copy, and holds its images back. Fictional senders on `example`
// domains only; nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record trusted-images-live \
//     --base-url http://localhost:5181

const NEWSLETTER = "Five tax dates before year end";
const STREAMS = ["primary", "updates", "receipts", "newsletters"] as const;

/** Where a conversation with `subject` is: its stream and id. */
async function find(
  page: Page,
  workspace: string,
  subject: string,
): Promise<{ stream: string; id: string }> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    for (const stream of STREAMS) {
      const list = (await api(
        page,
        "GET",
        `/bff/api/v1/w/${workspace}/conversations?stream=${stream}`,
      )) as { conversations?: { id: string; subject: string }[] };
      const found = list.conversations?.find((c) => c.subject === subject);
      if (found !== undefined) {
        return { stream, id: found.id };
      }
    }
    await pause(1000);
  }
  throw new Error(`no conversation "${subject}" in the demo mailbox`);
}

export default async function trustedImagesLive({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}`);

  const { stream, id } = await find(page, workspace, NEWSLETTER);
  await page.goto(`/w/${workspace}/inbox/${stream}/${id}`);
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: NEWSLETTER }).waitFor();
  await reader
    .getByText(/remote image hidden/u)
    .first()
    .waitFor();

  await reader.getByRole("button", { name: /^Always show images from The Weekly Ledger/u }).click();
  const dialog = page.getByRole("dialog", {
    name: "Always show images from hello@weekly-ledger.example",
  });
  await dialog.waitFor();
  await dialog.getByText(/vouch for the domain, not for this one person/u).waitFor();
  await caption(page, "The address choice says what the checks vouch for: its domain", 3000);
  await dialog.getByRole("button", { name: /Save smart rule/u }).click();
  await reader.getByText(/Images shown: (you|this inbox) trusts? The Weekly Ledger/u).waitFor();
  await caption(page, "Trusted: the Ledger's images show", 2000);

  // Remove the rule from outside this screen, as another device or member
  // would: straight through the API, with no navigation here.
  await caption(page, "Another device removes the rule…", 1800);
  await removeImageRules(page, workspace);
  await reader.getByRole("button", { name: /^Show images/u }).waitFor({ timeout: 20_000 });
  await caption(page, "…and this open message holds its images back again, with no reload", 3000);

  // Trust it again, leave it, and remove the rule while it is not on screen.
  await reader.getByRole("button", { name: /^Always show images from The Weekly Ledger/u }).click();
  await dialog.waitFor();
  await dialog.getByRole("button", { name: /Save smart rule/u }).click();
  await reader.getByText(/Images shown: (you|this inbox) trusts? The Weekly Ledger/u).waitFor();
  await caption(page, "Trusted again; now leave the newsletter", 1800);
  await page.keyboard.press("Escape");
  await pause(800);
  await caption(page, "The rule is removed elsewhere while it is closed…", 1800);
  await removeImageRules(page, workspace);
  await pause(1500);
  await page
    .getByRole("button", { name: /The Weekly Ledger/u })
    .first()
    .click();
  await reader.getByRole("button", { name: /^Show images/u }).waitFor({ timeout: 20_000 });
  await caption(
    page,
    "…opened again, it waits for a fresh copy: the images never load from the kept one",
    3000,
  );
}

/** Remove every rule that shows images, as another device or member would. */
async function removeImageRules(page: Page, workspace: string): Promise<void> {
  const { rules } = (await api(page, "GET", `/bff/api/v1/w/${workspace}/rules`)) as {
    rules: { id: string; state?: string }[];
  };
  const imageRules = rules.filter(
    (rule) => rule.state !== "removed" && JSON.stringify(rule).includes("show_images"),
  );
  for (const rule of imageRules) {
    await api(page, "DELETE", `/bff/api/v1/w/${workspace}/rules/${rule.id}`);
  }
}
