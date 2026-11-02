import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Signatures (campbooks-cloud#393), in Storybook with demo data only: the
// Settings editor, then a reply carrying the signature. Kept in memory; nothing
// is sent.
//   pnpm --dir clients storybook
//   pnpm --dir clients record signatures --base-url http://localhost:6006
const SETTINGS = "/iframe.html?id=screens-mailboxes--your-signature&viewMode=story";
const REPLY = "/iframe.html?id=screens-inbox--reply-draft-signature&viewMode=story";

export default async function signatures({ page }: ScenarioContext): Promise<void> {
  // Settings → Mailboxes → Your signature.
  await page.goto(SETTINGS);
  const box = page.getByRole("textbox", { name: "Your signature for ana@example.com" });
  await box.waitFor();
  await caption(page, "Settings → Mailboxes: your own signature for each mailbox", 2400);
  // Added to, as the member would (the editor sits on its element).
  // A script string, called here: a string handed to `locator.evaluate` is
  // only evaluated, never called with the element.
  await page.evaluate(`(() => {
    const box = document.querySelector(
      '[role="textbox"][aria-label="Your signature for ana@example.com"]');
    box.editor.chain().focus().selectAll().insertContent([
      { type: 'text', text: 'Ana Duarte', marks: [{ type: 'bold' }] },
      { type: 'hardBreak' },
      { type: 'text', text: 'Duarte & Pessoa · ' },
      { type: 'text', text: 'duarte.example', marks: [{ type: 'link', attrs: { href: 'https://duarte.example' } }] },
    ]).run();
  })()`);
  await page.getByText(/ of 2000 characters/u).waitFor();
  await caption(page, "Bold, italic and links, up to 2000 characters", 2400);
  await page.getByRole("button", { name: "Save" }).click();
  await page.getByText("Signature saved").waitFor();
  await caption(page, "Save, or ⌘↵: “Signature saved”", 2000);

  // A reply carries it.
  await page.goto(REPLY);
  const group = page.getByRole("group", { name: "Signature" });
  await group.waitFor();
  await caption(page, "A new reply carries your signature under the text, muted", 2600);
  await group.getByRole("button", { name: "Leave off" }).click();
  await group.getByRole("button", { name: "Add signature" }).waitFor();
  await caption(page, "Leave off drops it from this message", 2200);
  await group.getByRole("button", { name: "Add signature" }).click();
  await group.getByText("Rivera Lettings").waitFor();
  await caption(page, "Add signature brings it back; Edit in Settings changes it", 2600);
  await pause(600);
}
