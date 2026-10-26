import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Pages kept in step with Notion (campbooks-cloud#376), in Storybook with a
// small in-memory fake Notion and demo data only (Marta, Rui, Acme): connect
// Notion in Settings (Connect Notion leaves for Notion; in the story it comes
// back connected), import two pages (the sub-pages come along), open the
// roadmap, see a change from Notion waiting on a paragraph, choose Keep both,
// take it back with Undo, then stop syncing. On a phone: the same, in a
// column, nothing scrolling sideways.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-notion --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-notion--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** What a person does between a click and typing or the next click: let the page settle. */
const settle = () => pause(300);

async function settings(page: Page) {
  await page.goto(story("not-connected-idle"));
  await page.getByRole("button", { name: "Connect Notion" }).waitFor();
  await caption(page, "Settings: Notion shares only the pages you pick in Notion", 2600);
  await page.getByRole("button", { name: "Connect Notion" }).click();
  // Notion opens in the app's place; in the story it is the connected screen.
  await page.getByRole("link", { name: "Import pages from Acme" }).waitFor();
  await caption(page, "Connected: your Notion, named for its workspace", 2400);
}

async function importPages(page: Page, phone: boolean) {
  await page.getByRole("link", { name: "Import pages from Acme" }).click();
  await settle();
  const dialog = page.getByRole("dialog", { name: "Import from Notion" });
  await dialog.waitFor();
  await dialog.getByLabel("Product roadmap").waitFor();
  await caption(page, "Import from Notion: the pages you shared, with sub-pages", 2600);
  await dialog.getByLabel("Product roadmap").check();
  await settle();
  await caption(page, "Sub-pages come along", 2200);
  await dialog.getByLabel("Meeting notes").check();
  await settle();
  await caption(
    page,
    phone ? "Import 4 pages" : "Import 4 pages: two pages and the sub-pages",
    2200,
  );
  await dialog.getByRole("button", { name: /^Import \d+ pages/u }).click();
  await settle();
}

export default async function pagesNotion({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await settings(page);
  await importPages(page, phone);

  // The roadmap opens synced, with a change waiting.
  await page
    .getByText(/^Synced with Notion/u)
    .first()
    .waitFor();
  await caption(
    page,
    "Synced with Notion: the page says so, and Open in Notion is one click away",
    2600,
  );
  await page.getByText("Kept in Notion · Columns").scrollIntoViewIfNeeded();
  await caption(page, "Blocks Campbooks doesn’t have yet stay as they are in Notion", 2600);

  const keepBoth = page.getByRole("button", { name: "Keep both" });
  await keepBoth.scrollIntoViewIfNeeded();
  await caption(page, "The launch date changed in Notion while it was edited here", 2800);
  await keepBoth.click();
  await settle();
  await page.getByText(/Kept both, one under the other/u).waitFor();
  await caption(page, "Keep both: it goes to Notion on the next sync", 2400);

  await page.getByRole("button", { name: /Undo/u }).click();
  await settle();
  await page.getByRole("button", { name: "Keep both" }).waitFor();
  await caption(page, "Undo puts the choice back", 2200);

  await page.getByRole("button", { name: "Stop syncing" }).click();
  await settle();
  await page.getByRole("group", { name: /Stop keeping this page/u }).waitFor();
  await caption(page, "Stop syncing: the page stays, Notion is left alone", 2600);
  await page
    .getByRole("group", { name: /Stop keeping this page/u })
    .getByRole("button", { name: "Stop syncing" })
    .click();
  await settle();
  await page
    .getByText(/Stopped syncing with Notion/u)
    .first()
    .waitFor();
  await pause(1500);
}
