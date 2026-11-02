import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout explains a thread (campbooks-cloud#407), in Storybook with the
// in-memory server and demo data only (Ana, Rui Matos, Jonas Weber of Acme
// Studio, "Kickoff agenda"): nothing until you ask, Scout reading, the
// explanation with the messages each point comes from, a chip, Hide, coming
// back to one line, and Explain again after a new message. On a phone the
// card shows In short and On you, and the rest is one tap away.
//   pnpm --dir clients storybook
//   pnpm --dir clients record explain-thread --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-explain-a-thread--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** What a person does between a click and the next one: let the page settle. */
const settle = () => pause(300);

const CARD = "Scout’s explanation";

async function asking(page: Page, phone: boolean) {
  await page.goto(story("offered"));
  const note = page.getByRole("heading", { name: "Kickoff agenda" });
  await note.waitFor();
  await caption(page, "Nothing is explained until you ask: no card, no call", 2600);

  const [button] = await page.getByRole("button", { name: /Explain this thread/u }).all();
  await button?.click();
  await settle();
  const card = page.getByRole("region", { name: CARD });
  await card.waitFor();
  await page.getByText("Scout is reading the 9 messages…").waitFor();
  await caption(page, "Scout reads the nine messages; Stop only hides the card", 2400);

  await page.getByText(/Acme Studio wants the kickoff/u).waitFor({ timeout: 8000 });
  await caption(
    page,
    "In short, then who wants what, what was agreed, what is open, what is on you",
    3200,
  );

  if (phone) {
    await caption(page, "On a phone: In short and On you first", 2200);
    await card.getByText("Who wants what, agreed, open").click();
    await settle();
    await caption(page, "The rest is one tap away", 2200);
    return;
  }

  // A chip opens the message the point comes from.
  await card
    .getByRole("button", { name: /^Jonas · / })
    .first()
    .click();
  await settle();
  await caption(page, "A chip opens and scrolls to the message it comes from", 2600);

  // Hide folds it to one line.
  await card.getByRole("button", { name: "Hide the explanation" }).click();
  await page.getByText(/Your explanation from .* · from 9 messages/u).waitFor();
  await caption(page, "Hide folds the card to one line", 2200);
}

async function comingBack(page: Page) {
  await page.goto(story("coming-back"));
  await page.getByText(/Your explanation from .* · from 9 messages/u).waitFor();
  await caption(page, "Coming back: one line; nothing opens or is written by itself", 2800);
  await page.getByRole("button", { name: "Show", exact: true }).click();
  await page.getByRole("region", { name: CARD }).waitFor();
  await caption(page, "Show opens the card you asked for", 2200);
}

async function afterNewMessage(page: Page) {
  await page.goto(story("after-a-new-message"));
  await page.getByText(/Rui wrote since/u).waitFor();
  await caption(page, "A new message arrived: the line says so, and offers Explain again", 3000);
  await page.getByRole("button", { name: "Explain again", exact: true }).click();
  await page.getByText("Scout is reading the 10 messages…").waitFor();
  await caption(page, "Explain again is a new request; it never happens on its own", 2400);
  await page.getByText(/Acme Studio wants the kickoff/u).waitFor({ timeout: 8000 });
  await caption(page, "The new explanation reads all ten messages", 2400);
}

export default async function explainThread({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await asking(page, phone);
  await comingBack(page);
  await afterNewMessage(page);
  await pause(800);
}
