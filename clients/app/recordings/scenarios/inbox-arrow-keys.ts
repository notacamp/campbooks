import { existsSync, readFileSync, writeFileSync } from "node:fs";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Moving through the inbox with the arrow keys, on the real stack (see
// inbox.ts for the servers to start). Fictional person and mail only.
// Both widths run against one database: with INBOX_STATE set to a file path,
// the first width saves its session there and the second reuses it, instead
// of signing up a second workspace with no mailbox.

async function session(page: Page): Promise<string> {
  const file = process.env["INBOX_STATE"];
  if (file !== undefined && existsSync(file)) {
    const saved = JSON.parse(readFileSync(file, "utf8")) as {
      workspace: string;
      cookies: Parameters<ReturnType<Page["context"]>["addCookies"]>[0];
    };
    await page.context().addCookies(saved.cookies);
    return saved.workspace;
  }
  const workspace = await signedIn(page);
  if (file !== undefined) {
    writeFileSync(file, JSON.stringify({ workspace, cookies: await page.context().cookies() }));
  }
  return workspace;
}

export default async function inboxArrowKeys({ page }: ScenarioContext): Promise<void> {
  const workspace = await session(page);
  const shots = process.env["INBOX_SHOTS"];
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const shot = async (name: string) => {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/${name}-${phone ? "375" : "desktop"}.png` });
    }
  };
  const press = async (key: string, times = 1, ms = 450) => {
    for (let index = 0; index < times; index += 1) {
      await page.keyboard.press(key);
      await pause(ms);
    }
  };

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText("Q4 plan: can you review by Friday?").first().waitFor();
  if (!phone) {
    await caption(page, "The empty reader lists the keys: J or ↓, K or ↑", 2000);
    await shot("01-hints");
  }

  await caption(page, "↓ and ↑ move through the conversations", 900);
  await press("ArrowDown", 4);
  await press("ArrowUp", 2);
  await shot("02-cursor");
  await caption(page, "Moving never opens or marks anything read; Enter opens", 1400);
  await press("Enter", 1, 900);
  await page.waitForURL(/\/inbox\/primary\/[^/]+$/u);
  await shot("03-open");

  await caption(page, "In an open conversation ↓ scrolls it…", 900);
  await press("ArrowDown", 6, 200);
  await caption(page, "…and J or K go to the next or previous one", 900);
  await press("j", 1, 1200);
  await shot("04-next");

  await caption(page, "Esc goes back; the arrows carry on from there", 900);
  await press("Escape", 1, 700);
  await press("ArrowDown", 2);
  await press("ArrowUp", 1);

  await caption(page, "? lists every shortcut, with both keys", 900);
  await press("?", 1, 1800);
  await shot("05-sheet");
  await press("ArrowDown", 3, 250);
  await caption(page, "In a dialog, the arrows scroll the dialog", 1800);
  await press("Escape", 1, 600);
}
