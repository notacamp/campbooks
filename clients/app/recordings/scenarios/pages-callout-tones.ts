import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Callout tones on a page (campbooks-cloud#408), in Storybook with demo data
// only: info is blue, warning red, neutral grey, in light and then in dark.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-callout-tones --base-url http://localhost:6006
const story = (id: string, theme: string) =>
  `/iframe.html?id=screens-pages--${id}&viewMode=story&globals=theme:${theme}`;

export default async function pagesCalloutTones({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const id = phone ? "every-block-phone-idle" : "every-block-idle";
  for (const theme of ["light", "dark"]) {
    await page.goto(story(id, theme));
    const callout = page.getByText("The kickoff is on Thursday at 10:00.");
    await callout.waitFor();
    await callout.scrollIntoViewIfNeeded();
    await caption(
      page,
      theme === "light"
        ? "Info callouts are blue; warnings red, plain notes grey"
        : "The same three tones in dark",
      3500,
    );
  }
  await pause(500);
}
