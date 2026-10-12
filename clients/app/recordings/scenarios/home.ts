import { pause, type ScenarioContext } from "../context.ts";

// Opens the home page and waits for the system-status region, which renders
// even when the API is down.
export default async function home({ page }: ScenarioContext): Promise<void> {
  await page.goto("/");
  await page.getByRole("region").first().waitFor();
  await pause(1500);
}
