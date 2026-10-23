import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// The note offers Reply when it says a reply is needed (campbooks-cloud#306),
// in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record note-reply --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function noteReply({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("thread-idle"));
  const reader = page.getByRole("main", { name: "Conversation" });
  const note = reader.getByRole("region", { name: "Where things stand" });
  await note.waitFor();
  await caption(page, "The note says it needs a reply, and offers Reply right there", 2600);
  await note.getByRole("button", { name: "Reply", exact: true }).click();
  await caption(page, "Reply opens Scout's reply, the same as the reader's own Reply", 2800);
  await pause(800);
}
