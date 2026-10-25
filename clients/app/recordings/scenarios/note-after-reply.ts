import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// After you replied, the note says so and offers no Reply (campbooks-cloud#355),
// in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record note-after-reply --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function noteAfterReply({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("thread-idle"));
  const reader = page.getByRole("main", { name: "Conversation" });
  let note = reader.getByRole("region", { name: "Where things stand" });
  await note.waitFor();
  await caption(
    page,
    "Before: Maya's newest message needs a reply, so the note offers Reply",
    2800,
  );
  await page.goto(story("thread-replied"));
  note = page.getByRole("main", { name: "Conversation" }).getByRole("region", {
    name: "Where things stand",
  });
  await note.getByText("You replied on").waitFor();
  await caption(page, "After you replied: the note says so, and no longer offers Reply", 3200);
  await pause(800);
}
