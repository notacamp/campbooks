import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout asks when it isn't sure (campbooks-cloud#372), in Storybook with demo
// data only: Ana wrote to Águas do Ribatejo with the member in Cc, and Scout
// asks whose move it is instead of offering one Reply.
//   pnpm --dir clients storybook
//   pnpm --dir clients record home-scout-isnt-sure --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-home--${id}&viewMode=story`;

const LINE = "Ana Costa wrote to Águas do Ribatejo. You’re in Cc.";

export default async function homeScoutIsntSure({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story(phone ? "scout-isnt-sure-phone-recording" : "scout-isnt-sure-recording"));
  const line = page.getByText(LINE).first();
  await line.waitFor();
  await caption(
    page,
    "Ana wrote to someone else and copied you. Scout isn’t sure it’s yours",
    2600,
  );

  await line.click();
  await page.getByRole("group", { name: "Actions for Ana Costa" }).waitFor();
  await caption(page, "So it asks: nudge them, reply to Ana, or nothing for you", 2800);

  const how = page.getByText("How Scout decided").first();
  if (await how.isVisible()) {
    await how.click();
    await caption(page, "How it decided: a guess below the sure bar, and you’re only in Cc", 2600);
  }

  // 1 (or tap): someone else has the move. The conversation waits on them.
  if (phone) {
    await page
      .getByRole("button", { name: /^Nudge Águas do Ribatejo/u })
      .first()
      .click();
  } else {
    await page.keyboard.press("1");
  }
  await page.getByRole("dialog").waitFor();
  await caption(
    page,
    "Scout now waits on Águas do Ribatejo. The nudge is drafted, you decide",
    2800,
  );
  await page.keyboard.press("Escape");
  await pause(900);
  await caption(page, "It moved to Waiting on others. Undo is one key away", 2400);
  await pause(600);
}
