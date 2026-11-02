import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Settings → Connections (cloud #382), in Storybook with demo data only:
// Notion lives here, not in Mailboxes; the card says what it brings in and how
// it stays in step; Disconnect says what stops and what stays.
//   pnpm --dir clients storybook
//   pnpm --dir clients record settings-connections --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-notion--${id}&viewMode=story`;

export default async function settingsConnections({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("not-connected-idle"));
  await page.getByRole("button", { name: "Connect Notion" }).waitFor();
  await caption(page, "Connections: Notion, before it is connected", 2000);

  await page.goto(story("connections-connected-idle"));
  await page.getByText("Notion · Acme").waitFor();
  await caption(page, "Connected: what it brings in, and when Notion was last checked", 2600);

  await page.getByRole("button", { name: "Disconnect Acme" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText("What stops").waitFor();
  await caption(page, "Disconnect says what stops and what stays", 3000);
  await dialog.getByRole("button", { name: /Keep connected/u }).click();
  await caption(page, "Keep connected changes nothing", 1800);
}
