import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Notion databases as tables (campbooks-cloud#377), in Storybook with a small
// in-memory fake Notion and demo data only (Duarte & Pessoa's clients):
// import the Clients database as a table, see what Notion keeps (a lock with
// the reason on a formula and on people, "Campbooks only" on Threads, what
// stays in Campbooks when a column is made here), remove a column from both
// places and take it back, settle a cell changed in both places, and a column
// renamed in both places. On a phone: the dialog, the grid and the cards fit
// the width and nothing scrolls sideways but the grid.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-notion-databases --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-notion-databases--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** What a person does between a click and typing or the next click: let the page settle. */
const settle = () => pause(300);

/** The captions stay on screen long enough to read: about two seconds each. */
const SHORT = 2000;
const LONG = 2500;

async function importDatabase(page: Page, phone: boolean) {
  await page.goto(story(phone ? "import-with-databases-phone-idle" : "import-with-databases-idle"));
  const dialog = page.getByRole("dialog", { name: "Import from Notion" });
  await dialog.waitFor();
  await dialog.getByLabel("Clients", { exact: true }).waitFor();
  await caption(page, "Import from Notion: databases are listed beside pages", LONG);
  await dialog.getByLabel("Clients", { exact: true }).check();
  await settle();
  await caption(page, "Ticking a database imports it as a table", SHORT);
  await dialog.getByRole("button", { name: "Import 1 database" }).click();
  await settle();
}

async function lookAtTheTable(page: Page, phone: boolean) {
  await page
    .getByText(/^Synced with Notion/u)
    .first()
    .waitFor();
  await caption(page, "Synced with Notion: Open in Notion is one click away", LONG);
  const score = page.getByRole("button", { name: /^Score/u });
  await score.scrollIntoViewIfNeeded();
  await caption(
    page,
    "A formula stays Notion’s: a lock says why, and its cells can’t be edited",
    LONG,
  );
  const threads = page.getByRole("button", { name: /^Threads/u });
  await threads.scrollIntoViewIfNeeded();
  await caption(page, "Threads is Campbooks only: it is never sent to Notion", LONG);
  await threads.click();
  await settle();
  await page.getByText(/Threads is Campbooks only/u).waitFor();
  await caption(page, "The column’s menu says it too", SHORT);
  await page.keyboard.press("Escape");
  await settle();
  const add = page.getByRole("button", { name: "Add a column" });
  await add.scrollIntoViewIfNeeded();
  await add.click();
  await settle();
  await page.getByRole("menuitem", { name: /^Status/u }).waitFor();
  await caption(
    page,
    phone
      ? "Status stays in Campbooks"
      : "A status column made here stays in Campbooks: Notion can’t make one",
    LONG,
  );
  await page.keyboard.press("Escape");
  await settle();
}

async function removeColumn(page: Page) {
  const score = page.getByRole("button", { name: /^Score/u });
  await score.scrollIntoViewIfNeeded();
  await score.click();
  await settle();
  await page.getByRole("menuitem", { name: "Remove from both places…" }).click();
  await settle();
  const dialog = page.getByRole("dialog", { name: "Remove “Score” from both places?" });
  await dialog.waitFor();
  await caption(page, "Removing a synced column asks first: it goes from Notion too", LONG);
  await dialog.getByRole("button", { name: "Remove from both" }).click();
  await settle();
  await page.getByText("Column Score removed from both places").waitFor();
  await caption(page, "Campbooks keeps the values for 30 days", SHORT);
  await page.getByRole("button", { name: /Undo/u }).click();
  await settle();
  await page.getByRole("button", { name: /^Score/u }).waitFor();
  await caption(page, "Undo puts the column and its values back in both places", LONG);
}

async function cellChangedInBothPlaces(page: Page, phone: boolean) {
  await page.goto(story(phone ? "cell-conflict-phone-idle" : "cell-conflict-idle"));
  const flag = page.getByRole("button", {
    name: "Stage changed in Notion too: choose a version",
  });
  await flag.waitFor();
  await flag.scrollIntoViewIfNeeded();
  await caption(page, "Stage changed here and in Notion: 1 change to look at", LONG);
  await flag.click();
  await settle();
  const card = page.getByRole("region", { name: "A change from Notion in Stage" });
  await card.waitFor();
  await caption(page, "Notion’s value sits under ours; other cells merged on their own", LONG);
  await card.getByRole("button", { name: "Use Notion’s for Stage" }).click();
  await settle();
  await page
    .getByText(/Using Notion’s version/u)
    .first()
    .waitFor();
  await caption(page, "It goes to Notion on the next sync", SHORT);
  await page.getByRole("button", { name: /Undo/u }).click();
  await settle();
  await flag.waitFor();
  await caption(page, "Undo puts the choice back", SHORT);
}

async function columnChangedInBothPlaces(page: Page, phone: boolean) {
  await page.goto(story(phone ? "column-conflict-phone-idle" : "column-conflict-idle"));
  const banner = page.getByRole("region", { name: "Columns changed in both places" });
  await banner.waitFor();
  await caption(page, "A column renamed in both places: both names, above the table", LONG);
  await banner.getByRole("button", { name: "Use “Contract value”" }).click();
  await settle();
  await page.getByRole("columnheader", { name: "Contract value" }).waitFor();
  await caption(page, "Using Notion’s name; Undo is one click", LONG);
  await page.getByRole("button", { name: /Undo/u }).click();
  await settle();
  await page.getByRole("columnheader", { name: "Deal size" }).waitFor();
  await pause(SHORT);
}

export default async function pagesNotionDatabases({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await importDatabase(page, phone);
  await lookAtTheTable(page, phone);
  await removeColumn(page);
  await cellChangedInBothPlaces(page, phone);
  await columnChangedInBothPlaces(page, phone);
}
