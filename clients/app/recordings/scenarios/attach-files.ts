import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Attaching files to a reply (campbooks-cloud#391), in Storybook with demo
// data only: a reply draft open in its thread, kept in memory. Nothing is
// sent, and the files are in-memory buffers.
//   pnpm --dir clients storybook
//   pnpm --dir clients record attach-files --base-url http://localhost:6006
const STORY = "/iframe.html?id=screens-inbox--reply-draft-open&viewMode=story";

const PDF = {
  name: "lease-4b-signed.pdf",
  mimeType: "application/pdf",
  buffer: Buffer.concat([Buffer.from("%PDF-1.7\n"), Buffer.alloc(412_000, 120)]),
};
const SHEET = {
  name: "parking-form-P2-14.xlsx",
  mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  buffer: Buffer.alloc(38_400, 1),
};
const BLOCKED = {
  name: "setup.exe",
  mimeType: "application/octet-stream",
  buffer: Buffer.alloc(900, 2),
};

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function attachFiles({ page }: ScenarioContext): Promise<void> {
  const phone = isPhone(page);

  await page.goto(STORY);
  const composer = page.getByRole("region", { name: /^Reply to/u });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "A reply draft, with a way to attach files in its footer", 2200);

  // The picker: the button (or ⇧⌘A) opens it; here the files are chosen for it.
  const input = composer.getByTestId("composer-files");
  await composer.getByRole("button", { name: /^Attach files/u }).hover();
  await caption(page, "Attach (⇧⌘A) opens the file picker", 1800);
  await input.setInputFiles([PDF, SHEET]);
  await composer.getByText("lease-4b-signed.pdf").waitFor();
  await composer.getByText(/ of 25 MB/u).waitFor();
  await caption(page, "Each file shows its type, name and size, and the total of 25 MB", 2400);

  // A type that mail providers block is refused, naming the file.
  await input.setInputFiles([BLOCKED]);
  await page.getByText(/setup\.exe wasn.t attached/u).waitFor();
  await caption(page, "A file Gmail would refuse is turned down before it uploads", 2400);

  // Remove one.
  await composer.getByRole("button", { name: `Remove ${SHEET.name}` }).click();
  await composer.getByText(SHEET.name).waitFor({ state: "detached" });
  await caption(page, "Remove takes a file off the message", 1800);

  if (phone) {
    await pause(600);
    return;
  }

  // Dragging files over the composer: the drop zone (dispatched as the browser would).
  await composer.evaluate(`(section) => {
    const data = new DataTransfer();
    data.items.add(new File(['x'], 'photo.png', { type: 'image/png' }));
    section.dispatchEvent(new DragEvent('dragenter', { bubbles: true, dataTransfer: data }));
  }`);
  await composer.getByText("Drop to attach").waitFor();
  await caption(page, "Drag files over the composer to drop them in", 2200);
  await composer.evaluate(`(section) => {
    const data = new DataTransfer();
    data.items.add(new File(['x'], 'photo.png', { type: 'image/png' }));
    section.dispatchEvent(new DragEvent('dragleave', { bubbles: true, dataTransfer: data }));
  }`);
  await composer.getByText("Drop to attach").waitFor({ state: "detached" });
  await pause(600);
}
