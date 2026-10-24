import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Tasks (campbooks-cloud#43), in Storybook with demo data only: a member's
// open tasks, the new-task form, a task opened, Done with Undo, a task handed to a
// teammate, Home, and "Make it a task" in the reader.
//   pnpm --dir clients storybook
//   pnpm --dir clients record tasks --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tasks--${id}&viewMode=story`;

const LEASE = "Send the signed lease to Marta";
const BILL = "EDP electricity bill · €84.20";
const VENUE = "Book the venue for the team lunch";

/** Leave the field the cursor is in, so the list's keys work. */
const leaveField = (page: ScenarioContext["page"]) =>
  page.evaluate(`document.activeElement instanceof HTMLElement && document.activeElement.blur()`);

/** Move the cursor (J/K) onto the row whose text has `text`. */
async function cursorOn(page: ScenarioContext["page"], text: string): Promise<void> {
  for (let index = 0; index < 12; index += 1) {
    const cursor = page.locator("[data-selected]");
    const selected = (await cursor.count()) === 0 ? "" : await cursor.first().textContent();
    if (selected?.includes(text) === true) {
      return;
    }
    await page.keyboard.press("j");
    await pause(200);
  }
}

export default async function tasks({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story("list-idle"));
  await page.getByText(LEASE).first().waitFor();
  await caption(page, "Tasks: what is late, today, coming up, and what has no date", 2600);

  if (phone) {
    // Open a task (it replaces the list), go back, then Done with Undo.
    await page.getByText(LEASE).first().click();
    await page.getByText("When will you do it?").waitFor();
    await caption(page, "A task opened: when, where it came from, hand-off", 2400);
    await page.getByRole("button", { name: "Back" }).click();
    await page.getByText(BILL).first().waitFor();
    await page.getByRole("button", { name: `Mark paid: ${BILL}` }).click();
    await caption(page, "Paid: gone from the list, with Undo", 2200);
    await page.getByRole("button", { name: /^Undo/u }).click();
    await page.getByText(BILL).first().waitFor();
    await caption(page, "Undo puts it back", 2000);
    await page.getByText(VENUE).first().scrollIntoViewIfNeeded();
    await caption(page, "A task handed to Rui: With Rui Matos, and Take it back", 2600);
    await page.getByRole("button", { name: "Take it back" }).first().click();
    await caption(page, "Taken back: it is yours again, at once", 2400);
    return;
  }

  // The new-task form: N T opens it in the right pane; a title, a due chip, Enter.
  await page.keyboard.press("n");
  await pause(150);
  await page.keyboard.press("t");
  const title = page.getByRole("textbox", { name: "Task title" });
  await title.waitFor();
  await caption(page, "N then T: a new task, in the right pane", 1400);
  await title.pressSequentially("Renew the parking permit", { delay: 45 });
  await page.getByRole("button", { name: "Tomorrow" }).click();
  await caption(page, "A due chip: Tomorrow", 1400);
  await title.focus();
  await page.keyboard.press("Enter");
  await page.getByText("Renew the parking permit").first().waitFor();
  await caption(page, "Enter adds it: the list updates and the form is ready for the next", 2600);
  await page.keyboard.press("Escape");
  await pause(400);
  await leaveField(page);

  // Open one with J and Enter; S offers Later.
  await cursorOn(page, LEASE);
  await caption(page, "J and K move, Enter opens", 1400);
  await page.keyboard.press("Enter");
  await page.getByText("When will you do it?").waitFor();
  await pause(600);
  await page.keyboard.press("s");
  await page.getByRole("button", { name: "Next week" }).waitFor();
  await caption(page, "S offers Later: tonight, tomorrow, next week or a date", 2800);
  await page.keyboard.press("Escape");
  await pause(500);

  // E on the bill: Paid, with Undo (Z).
  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press("k");
    await pause(120);
  }
  await cursorOn(page, BILL);
  await page.keyboard.press("e");
  await page.getByText(`Paid · ${BILL}`).first().waitFor();
  await caption(page, "E marks the bill Paid. Campbooks never pays, it only marks it", 2800);
  await page.keyboard.press("z");
  await page.getByText(BILL).first().waitFor();
  await caption(page, "Z undoes it", 2000);

  // A task handed to a teammate.
  await page.getByText(VENUE).first().scrollIntoViewIfNeeded();
  await caption(
    page,
    "Handed to Rui: it says With Rui Matos, and Take it back is on the row",
    3000,
  );
  await page.getByRole("button", { name: "Take it back" }).first().click();
  await caption(page, "Taken back: it is yours again, at once", 2400);

  // Home: a task in Do and one in Pay.
  await page.goto(story("home-idle"));
  await page.getByText(LEASE).first().waitFor();
  await caption(page, "Home: tasks in Do and Pay, beside the emails", 3000);

  // The reader: Make it a task.
  await page.goto(story("reader-idle"));
  await page.getByRole("heading", { level: 2 }).first().waitFor();
  await pause(800);
  await page.keyboard.press("n");
  await pause(150);
  await page.keyboard.press("t");
  await page.getByRole("textbox", { name: "Title" }).waitFor();
  await caption(page, "N then T in an email: Make it a task, the subject as the title", 2600);
  await page.getByRole("button", { name: /^Add task/u }).click();
  await page.getByText("Added to your tasks").first().waitFor();
  await caption(page, "Added: the form closes; the toast has Undo and Open", 3000);
}
