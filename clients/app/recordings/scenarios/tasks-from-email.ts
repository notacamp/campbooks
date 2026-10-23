import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Tasks Scout made from emails (campbooks-cloud#43), in Storybook with demo
// data only: Home shows a Pay and a Do task found in emails, and a task's
// detail says Scout found it in the email.
//   pnpm --dir clients storybook
//   pnpm --dir clients record tasks-from-email --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tasks--${id}&viewMode=story`;

const BILL = "Pay invoice 7781 · €84.20";
const LEASE = "Send the signed lease";

export default async function tasksFromEmail({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("found-on-home-idle"));
  await page.getByText(BILL).first().waitFor();
  await caption(page, "Home: Scout turned two emails into tasks, one Pay and one Do", 3200);
  await page.getByText(LEASE).first().scrollIntoViewIfNeeded();
  await caption(page, "Each shows once: the email's own line gives way to its task", 2800);

  await page.goto(story("found-by-scout-idle"));
  await page.getByText("Scout found this in the email").waitFor();
  await caption(page, "The task says where it came from: Scout found it in the email", 3200);
  await page.getByRole("link", { name: "Open the email" }).scrollIntoViewIfNeeded();
  await caption(page, "Open the email is one click away", 2400);
  await pause(600);
}
