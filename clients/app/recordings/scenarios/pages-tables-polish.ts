import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A table's cells end in an ellipsis with the whole value on hover, and the
// phone's "New row" has one "+" (campbooks-cloud#387), in Storybook with demo
// data only: a narrow number column on the desktop, then the phone's footer.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-tables-polish --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tables--${id}&viewMode=story`;

export default async function pagesTablesPolish({ page }: ScenarioContext): Promise<void> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(story("narrow-number-idle"));
  const number = page.getByText("€9,600.00").first();
  await number.waitFor();
  await caption(page, "A narrow number column ends in an ellipsis, not a clipped digit", 2800);
  await number.hover();
  await caption(page, "Hover it: the whole value is in its title", 2800);
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto(story("narrow-number-phone-idle"));
  await page.getByRole("button", { name: /^New row/u }).waitFor();
  await caption(page, "On a phone, New row has one +, and Add a column stays in the header", 3200);
  await page.getByRole("button", { name: "Add a column" }).click();
  await caption(page, "The add-column + still opens its menu", 2400);
}
