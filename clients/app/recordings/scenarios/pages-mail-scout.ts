import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Mail and Scout in Pages (campbooks-cloud#364), in Storybook with the
// in-memory pages server and demo data only (Ana, Rui, Jonas Weber's "Kickoff
// agenda" thread, Acme Studio, "Client onboarding"): save the thread to a page
// (pick it by name, see who will read a copy, save, Undo), then Scout's page
// card (readers and Notion, Show all, Add to page, Undo), then Ask Scout on a
// page. On a phone the dialog is a sheet and the card's buttons wrap.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-mail-scout --base-url http://localhost:6006
const story = (id: string) =>
  `/iframe.html?id=screens-pages-from-mail-and-scout--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** What a person does between a click and typing or the next click: let the page settle. */
const settle = () => pause(300);

const TITLE = "Save “Kickoff agenda” to a page";

async function saveToPage(page: Page, phone: boolean) {
  await page.goto(story(phone ? "dialog-phone-idle" : "dialog-idle"));
  const dialog = page.getByRole("dialog", { name: TITLE });
  await dialog.waitFor();
  await caption(page, "Save to page: pages that already link the thread come first", 2500);

  // Find a page by name.
  const search = dialog.getByLabel("Find a page by name", { exact: true });
  await search.click();
  await settle();
  await search.pressSequentially("acme", { delay: 120 });
  await caption(page, "Pages are found by name", 2000);
  await dialog.getByRole("option", { name: /^Acme Studio/u }).click();
  await settle();

  // A copy says who will read it.
  await dialog.getByRole("radio", { name: /A copy of the messages/u }).click({ force: true });
  await dialog.getByText("Rui Matos will read it").waitFor();
  await caption(page, "A copy says who will read it, before you save", 2600);

  // A live block in Client onboarding, saved, then taken back.
  await search.fill("");
  await settle();
  await dialog.getByRole("option", { name: /^Client onboarding/u }).click();
  await settle();
  await dialog.getByRole("radio", { name: /A live thread block/u }).click({ force: true });
  await settle();
  await dialog.getByRole("checkbox", { name: "Include attachments (2)" }).click({ force: true });
  await caption(page, "The thread stays live; the attachments come along", 2400);
  await dialog.getByRole("button", { name: /^Save/u }).click();
  await page.getByText("Saved to Client onboarding").first().waitFor();
  await caption(page, "Open the page, or take it back", 2300);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await settle();
}

async function pageCard(page: Page, phone: boolean) {
  await page.goto(story(phone ? "card-phone-idle" : "card-idle"));
  const card = page.getByRole("region", { name: "Add to “Client onboarding”" });
  await card.waitFor();
  await caption(
    page,
    "Scout proposes a section: who will read it, Notion too, and all of the text",
    2800,
  );
  await card.getByRole("button", { name: "Show all" }).click();
  await settle();
  await caption(page, "Show all folds a long section back out", 2200);
  await card.getByRole("button", { name: "Add to page" }).click();
  await card.getByText("Added", { exact: true }).waitFor();
  await caption(page, "Added: open the page, or undo", 2400);
  await card.getByRole("button", { name: "Undo" }).click();
  await card.getByText("Undone.", { exact: true }).waitFor();
  await settle();
}

async function askScout(page: Page) {
  await page.goto(story("ask-scout-idle"));
  await page.getByRole("button", { name: "More" }).click();
  await settle();
  await caption(page, "Ask Scout is in the page's More menu: the toolbar stays one row", 2600);
  await page.getByRole("button", { name: /^Ask Scout/u }).click();
  await page.getByText("Page: Client onboarding").waitFor();
  await caption(page, "A chat about this page", 2300);
}

export default async function pagesMailScout({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await saveToPage(page, phone);
  await pageCard(page, phone);
  if (!phone) {
    await askScout(page);
  }
  await pause(800);
}
