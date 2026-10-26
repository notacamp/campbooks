import { pause, type ScenarioContext } from "../context.ts";

// The design system as the @notacamp/ui package (clients/ui), in Storybook
// (demo data only): its own components, then the app's patterns built on it.
//   pnpm --dir clients storybook
//   pnpm --dir clients record design-system-package --base-url http://localhost:6006
const story = (id: string, globals = "") =>
  `/iframe.html?id=${id}&viewMode=story${globals === "" ? "" : `&globals=${globals}`}`;

export default async function designSystemPackage({ page }: ScenarioContext): Promise<void> {
  // Components that now live in the package.
  await page.goto(story("components-button--all-variants"));
  await pause(1800);
  await page.goto(story("components-commandmenu--filtering"));
  await pause(2000);
  await page.goto(story("components-dialog--interactive"));
  await pause(2000);
  await page.goto(story("components-datepicker--open"));
  await pause(1800);
  await page.goto(story("components-note--suggestion", "theme:dark"));
  await pause(1800);

  // The app's screens, drawn with the package's components and tokens.
  await page.goto(story("patterns-home--business"));
  await page.getByRole("heading", { level: 1 }).waitFor();
  await pause(1200);
  await page.keyboard.press("j");
  await pause(600);
  await page.keyboard.press("?");
  await pause(2000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+k");
  await page.keyboard.type("inb", { delay: 120 });
  await pause(1500);
  await page.keyboard.press("Escape");

  // Every theme is a token set over the same components.
  await page.goto(story("patterns-themes--all-themes"));
  await pause(2000);
  await page.mouse.wheel(0, 900);
  await pause(1800);
}
