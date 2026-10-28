import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// The roomier composer (campbooks-cloud#390) on the real stack, as the `inbox`
// scenario runs it (see its header for the commands). Fictional mail only;
// nothing is sent. Run it at 375 px and at desktop width:
//
//   INBOX_BASE_URL=http://localhost:5191 pnpm --dir clients record roomier-composer \
//     --base-url http://localhost:5191

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

const Q4 = "Q4 plan: can you review by Friday?";

export default async function roomierComposer({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const mod = process.platform === "darwin" ? "Meta" : "Control";

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(Q4).first().click();
  await page.getByRole("heading", { name: Q4 }).waitFor();
  await (phone
    ? page.getByRole("button", { name: "Reply", exact: true }).last().click()
    : page.keyboard.press("r"));
  await page.getByRole("button", { name: "Write my own" }).click();

  const composer = page.getByRole("region", { name: /^Reply to/u });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "More room to write, and the fields are the size of the text");

  // Bold a selection: it shows as bold, not as asterisks.
  await body.pressSequentially("I will renew for a year and read the lease.", { delay: 25 });
  await page.keyboard.press("Home");
  for (let i = 0; i < 7; i += 1) {
    await page.keyboard.press("ArrowRight");
  }
  for (let i = 0; i < 15; i += 1) {
    await page.keyboard.press("Shift+ArrowRight");
  }
  await page.keyboard.press(`${mod}+b`);
  await caption(page, "Select words and press the formatting key: they show as bold");

  // A link with words, by key.
  await page.keyboard.press("End");
  await page.keyboard.press(`Shift+ArrowLeft`);
  for (let i = 0; i < 4; i += 1) {
    await page.keyboard.press("Shift+ArrowLeft");
  }
  await page.keyboard.press(`${mod}+Shift+k`);
  await composer.getByRole("textbox", { name: "Address" }).fill("https://example.com/lease");
  await caption(page, "Link takes the words and an address", 1800);
  await composer.getByRole("button", { name: "Add link" }).click();

  // A bullet list.
  await body.press("End");
  await body.press("Enter");
  await page.keyboard.press(`${mod}+Shift+8`);
  await body.pressSequentially("parking P2-14", { delay: 25 });
  await body.press("Enter");
  await body.pressSequentially("rent on the 1st", { delay: 25 });
  await caption(page, "A bulleted list, on its own key");
  await composer.getByText("Saved").waitFor();

  if (phone) {
    await composer.getByRole("button", { name: "More formatting" }).click();
    await caption(page, "On a phone the toolbar stays one row: the rest is under More", 2200);
    await page.keyboard.press("Escape");
    return;
  }

  // Expand and back.
  await page.keyboard.press(`${mod}+Shift+e`);
  await composer.getByRole("button", { name: "Shrink" }).waitFor();
  await caption(page, "Expand opens the same draft as a sheet, cursor and all", 2200);
  await body.pressSequentially(" Thank you!", { delay: 25 });
  await page.keyboard.press("Escape");
  await composer.getByRole("button", { name: "Expand" }).waitFor();
  await pause(600);
  await caption(page, "Esc puts it back where it was", 1800);
}
