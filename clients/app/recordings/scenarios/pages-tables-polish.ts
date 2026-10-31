import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A table's cells end in an ellipsis with the whole value on hover, and the
// phone's "New row" has one "+" (campbooks-cloud#387), in Storybook with demo
// data only: a narrow number column on the desktop, then the phone's footer.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-tables-polish --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tables--${id}&viewMode=story`;

export default async function pagesTablesPolish({ page }: ScenarioContext): Promise<void> {
  // The recorder runs this at 375px and on the desktop: the same story shows
  // the narrow number column and the footer's New row at each width.
  await page.goto(story("narrow-number-idle"));
  await page.getByText("€9,600.00").first().waitFor();
  await caption(page, "A narrow number column ends in an ellipsis, not a clipped digit", 2800);
  await page.getByText("€9,600.00").first().hover();
  await caption(page, "The whole value is in the cell's title", 2400);
  await page
    .getByRole("button", { name: /^New row/u })
    .first()
    .scrollIntoViewIfNeeded();
  await caption(page, "New row has one +, and Add a column stays in the header", 3000);
}
