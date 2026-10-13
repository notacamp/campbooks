import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// Attachments on the real stack, as the `inbox` scenario runs it (see its
// header for the commands: the fake Google of `scripts/fake-provider.ts` with
// the demo Gmail of `scripts/fake-gmail.ts`, whose newest message carries a
// photo, a PDF, an HTML file, an inline logo and a remote route map).
// Nothing is intercepted: the bytes come from blob storage through
// `conversations.get_attachment` and the BFF.
//
//   INBOX_BASE_URL=http://localhost:5105 pnpm --dir clients record attachments \
//     --base-url http://localhost:5105
//
// Fictional people and files only (`.example`); no token is shown.

const SUBJECT = "Photos and tickets for the harbour walk";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** The message's sandboxed frame. */
function body(page: Page) {
  return page.locator('iframe[title*="Ana Lopes"]').last().contentFrame();
}

export default async function attachments({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const shots = process.env["INBOX_SHOTS"];
  const shot = async (name: string) => {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/${name}-${phone ? "375" : "desktop"}.png` });
    }
  };

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(SUBJECT).first().waitFor({ timeout: 60_000 });
  await caption(page, "A message from Ana with a photo, boat tickets and an itinerary");
  await page.getByText(SUBJECT).first().click();
  await page.getByRole("heading", { name: SUBJECT }).waitFor();
  await body(page).getByRole("img", { name: "Brightloop" }).waitFor();
  await caption(page, "Her logo is part of the message (cid:): it shows without asking", 2200);
  await shot("01-message");

  // The remote route map is held back: a placeholder, not a broken image.
  if (phone) {
    await page.mouse.wheel(0, 300);
  }
  await caption(page, "The route map is remote: held back, a quiet placeholder in its place", 2400);
  await shot("02-placeholder");

  const list = page.getByRole("region", { name: "Attachments" });
  await list.getByRole("listitem").last().scrollIntoViewIfNeeded();
  if (phone) {
    // Clear of the floating Reply / Done bar.
    await page.mouse.wheel(0, 240);
  }
  await caption(page, "Each file: its type, name and size, Preview and Download", 2200);
  await shot("03-list");

  // Preview the photo.
  if (phone) {
    await list.getByRole("button", { name: "harbour-at-dusk.png", exact: true }).click();
  } else {
    await caption(page, "V moves through the attachments…", 1200);
    await page.keyboard.press("v");
    await pause(600);
    await caption(page, "…Enter opens the focused one", 1200);
    await page.keyboard.press("Enter");
  }
  const dialog = page.getByRole("dialog", { name: "Preview of harbour-at-dusk.png" });
  await dialog.getByRole("img", { name: "harbour-at-dusk.png" }).waitFor();
  await caption(page, "The photo, shown as a picture: never as a page", 2400);
  await shot("04-preview");
  await (phone
    ? dialog.getByRole("button", { name: "Close" }).click()
    : page.keyboard.press("Escape"));
  await dialog.waitFor({ state: "hidden" });

  // Download the PDF.
  const saving = page.waitForEvent("download");
  await list.getByRole("button", { name: "Download boat-tickets.pdf" }).click();
  const download = await saving;
  await caption(page, `Downloaded ${download.suggestedFilename()}`, 2000);

  // HTML is download-only.
  await list.getByRole("button", { name: "itinerary.html", exact: true }).hover();
  await caption(page, "itinerary.html is HTML: Download only, never opened in the app", 2400);
  await shot("05-html");

  // Show images.
  await page.mouse.wheel(0, -400);
  await caption(page, phone ? "Show images" : "Show images (Shift+I)", 1200);
  await page.getByRole("button", { name: "Show images" }).first().click();
  await body(page).getByRole("img", { name: "Route map" }).waitFor();
  await pause(1200);
  await caption(page, "Now the route map loads from the walking site", 2400);
  await shot("06-shown");
}
