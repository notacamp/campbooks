import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A shared mailbox on Scout's "isn't sure" card (campbooks-cloud#379), in
// Storybook with demo data only: Ana wrote to "Geral" <geral@…>, and the card
// names the organisation and offers to nudge it.
//   pnpm --dir clients storybook
//   pnpm --dir clients record home-shared-mailbox --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-home--${id}&viewMode=story`;

export default async function homeSharedMailbox({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(
    story(
      phone
        ? "scout-isnt-sure-shared-mailbox-phone-recording"
        : "scout-isnt-sure-shared-mailbox-recording",
    ),
  );
  const line = page.getByText(/^Ana Costa wrote to Aguas Ribatejo/u).first();
  await line.waitFor();
  await caption(page, "Ana wrote to a shared mailbox that only calls itself “Geral”", 2600);
  await line.click();
  await page
    .getByRole("button", { name: /^Nudge Aguas Ribatejo/u })
    .first()
    .waitFor();
  await caption(page, "Scout names the organisation from its address, and you can nudge it", 2800);
  await pause(800);
}
