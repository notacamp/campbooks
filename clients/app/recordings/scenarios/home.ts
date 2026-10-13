import { pause, type ScenarioContext } from "../context.ts";

// Opens the app's root. Without a server it shows "Can't reach Campbooks";
// signed out, the landing; signed in, the last workspace's Home. Every one of
// those has a page heading. For the full flow with demo data, record
// `first-run` instead.
export default async function home({ page }: ScenarioContext): Promise<void> {
  await page.goto("/");
  await page.getByRole("heading", { level: 1 }).waitFor();
  await pause(1500);
}
