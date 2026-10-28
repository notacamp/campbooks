import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Teach once from Scout's "whose move" answers (campbooks-cloud#378), in
// Storybook with demo data only: the member answers Nudge for Ana's mail to
// someone else a second time, and Scout offers to wait on them from now on.
//   pnpm --dir clients storybook
//   pnpm --dir clients record home-wait-rule-offer --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-home--${id}&viewMode=story`;

const LINE = "Ana Costa wrote to Águas do Ribatejo. You’re in Cc.";

export default async function homeWaitRuleOffer({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(
    story(
      phone ? "scout-isnt-sure-wait-offer-phone-recording" : "scout-isnt-sure-wait-offer-recording",
    ),
  );
  const line = page.getByText(LINE).first();
  await line.waitFor();
  await caption(page, "You nudged Ana’s mail to someone else before. Here is another", 2600);

  await line.click();
  await page.getByRole("group", { name: "Actions for Ana Costa" }).waitFor();
  await (phone
    ? page
        .getByRole("button", { name: /^Nudge Águas do Ribatejo/u })
        .first()
        .click()
    : page.keyboard.press("1"));
  await page.getByRole("dialog", { name: "Nudge Águas" }).waitFor();
  await caption(page, "Nudge them: Scout waits on them, and the nudge is yours to send", 2600);
  await page.keyboard.press("Escape");

  const offer = page.getByRole("dialog", {
    name: "When Ana writes to someone else with you in Cc, should Scout wait on them for you?",
  });
  await offer.waitFor();
  await caption(page, "The second time, Scout offers to do this for you every time", 3000);

  await (phone
    ? offer.getByRole("button", { name: /Yes, always/u }).click()
    : page.keyboard.press("1"));
  await page.getByRole("dialog", { name: "A rule for when Ana writes to someone else" }).waitFor();
  await caption(page, "Yes opens the rule. Nothing is saved until you save it", 3000);
  await pause(800);
}
