import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Tables in Pages (campbooks-cloud#365), in Storybook with the in-memory
// table server and demo data only (Duarte & Pessoa's clients): edit a number
// and a stage in place with Undo, add a column from the type menu, filter,
// open a row beside the table, move a card on the board with its Move to
// menu, and a table inside a page. On a phone: the grid scrolls inside its
// box and the page never does; a row opens full screen.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-tables --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tables--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** What a person does between a click and typing or the next click: let the page settle. */
const settle = () => pause(300);

/** Columns of the demo table, in order. */
const COLUMN = { name: 0, stage: 1, owner: 2, next: 3, value: 4, threads: 5, signed: 6 } as const;

const cell = (page: Page, row: string, column: number) =>
  page.getByRole("row", { name: row }).getByRole("gridcell").nth(column);

async function editValue(page: Page) {
  await cell(page, "Northwind Bakery", COLUMN.value).click();
  await settle();
  await page.keyboard.press("Enter");
  await settle();
  await page.keyboard.press("Control+a");
  await page.keyboard.type("5200", { delay: 90 });
  await caption(page, "Enter edits a cell, Esc cancels: the value changes where it is", 2300);
  await page.keyboard.press("Enter");
  await page.getByText("Value changed").waitFor();
  await caption(page, "Every change can be undone", 2200);
  await page.getByRole("button", { name: /Undo/u }).click();
  await settle();
}

async function pickStage(page: Page) {
  await cell(page, "Harbour Physio", COLUMN.stage).click();
  await settle();
  await page.keyboard.press("Enter");
  const options = page.getByRole("combobox", { name: "Search options" });
  await options.waitFor();
  await caption(page, "Options are found by name; a new one is made from what you type", 2400);
  await options.fill("Proposal");
  await settle();
  await page.getByRole("option", { name: "Proposal sent" }).click();
  await page.getByText("Stage changed").waitFor();
  await settle();
}

async function addColumn(page: Page) {
  await page.getByRole("button", { name: "Add a column" }).click();
  await settle();
  await page.getByLabel("Column name").fill("Region");
  await caption(page, "A new column: Basic, Links, and Filled in for you", 2400);
  await page.getByRole("menuitem", { name: /^Select/u }).click();
  await page.getByRole("columnheader", { name: "Region" }).waitFor();
  await settle();
  await caption(page, "Region is a select: its options come from the cells", 2200);
}

async function filterRows(page: Page) {
  await page.getByRole("button", { name: /^Filter/u }).click();
  await settle();
  await page.getByRole("button", { name: "Add condition" }).click();
  await settle();
  const condition = page.getByRole("group", { name: "Condition 2" });
  await condition.getByLabel("Property").selectOption({ label: "Value" });
  await condition.getByLabel("Condition").selectOption({ label: "is greater than" });
  await condition.getByLabel("Value").fill("3000");
  await caption(page, "Filter by any column: the operators come from its type", 2400);
  await page.getByRole("button", { name: "Apply" }).click();
  await page.getByRole("row", { name: "Harbour Physio" }).waitFor({ state: "detached" });
  await settle();
}

async function openRow(page: Page, phone: boolean) {
  await page.getByRole("button", { name: "Open Acme Studio" }).click();
  const peek = page.getByRole("dialog", { name: "Row" });
  await peek.waitFor();
  await caption(
    page,
    phone
      ? "A row is a page: full screen on a phone"
      : "A row is a page: its properties and its notes",
    2400,
  );
  await peek.getByRole("checkbox", { name: "Signed" }).click();
  await settle();
  await page.keyboard.press("Escape");
  await peek.waitFor({ state: "detached" });
}

async function moveCard(page: Page) {
  await page.getByRole("tab", { name: "By stage" }).click();
  await page.getByRole("list", { name: "Kickoff" }).waitFor();
  await caption(page, "The board: a column per stage, with its count and sum", 2400);
  await page.getByRole("button", { name: "Move Harbour Physio to" }).click();
  await settle();
  await caption(page, "Drag a card, or use Move to: the keyboard way", 2300);
  await page.getByRole("menuitemradio", { name: "Active" }).click();
  await page.getByText("Stage changed").first().waitFor();
  await settle();
}

export default async function pagesTables({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story(phone ? "table-phone-idle" : "table-idle"));
  await page.getByRole("grid").waitFor();
  await caption(
    page,
    phone
      ? "Each row is a page. The grid scrolls inside its box"
      : "Each row is a page. Columns are typed, so they sort, filter and group",
    2500,
  );

  if (phone) {
    // The grid scrolls sideways inside its own box; the page stays put.
    // A script string: the recordings are type-checked without the DOM types.
    await page.evaluate(
      `document.querySelector('[role="grid"]')?.parentElement?.scrollTo({ left: 400, behavior: "smooth" })`,
    );
    await pause(900);
    await openRow(page, true);
    await page.goto(story("board-phone-idle"));
    await page.getByRole("list", { name: "Kickoff" }).waitFor();
    await caption(page, "The board scrolls by column", 2300);
    await pause(800);
    return;
  }

  await editValue(page);
  await pickStage(page);
  await addColumn(page);
  await filterRows(page);
  await openRow(page, false);
  await moveCard(page);

  await page.goto(story("inline-table-idle"));
  await page.getByRole("grid").waitFor();
  await caption(
    page,
    "/table puts a table in any page, and /link to table shows an existing one",
    2500,
  );
  await pause(800);
}
