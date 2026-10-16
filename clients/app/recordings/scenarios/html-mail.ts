import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// HTML mail in the reader (campbooks-cloud#160), on the real stack as the
// `inbox` scenario runs it, with the fake Google started with `--html-mail`:
// its Gmail mailbox then also holds the sanitiser's fixture emails (a
// newsletter, a receipt, a fixed 600px sale, Outlook and Gmail replies and a
// hostile message). Nothing is intercepted.
//
//   pnpm --dir clients fake-google --port 9071 --html-mail
//   (server and Vite as in `inbox.ts`)
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record html-mail \
//     --base-url http://localhost:5181
//
// `HTML_MAIL_LABEL` prefixes every caption (e.g. "Before" / "After", to
// record both sides of a change); `INBOX_SHOTS` saves a screenshot per step.
// Fictional senders and fixtures only (`.example`).

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** The fixture emails, in the order shown, and what each one shows. */
const STEPS: { subject: string; says: string; images?: boolean }[] = [
  {
    subject: "A simple call to action",
    says: "A newsletter styled by a style sheet and classes",
  },
  {
    subject: "Your Harbourline receipt R-20418",
    says: "A receipt in nested tables",
  },
  {
    subject: "Thirty per cent off waterproofs",
    says: "A fixed 600px sale: images held back until Show images",
    images: true,
  },
  { subject: "Re: Harbour walk figures", says: "A reply written in Outlook" },
  { subject: "Re: Second instalment", says: "A reply written in Gmail, with the quote" },
  {
    subject: "You have won (a hostile test message)",
    says: "A hostile message: scripts, forms, beacons and pixels, all gone",
  },
];

export default async function htmlMail({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const label = process.env["HTML_MAIL_LABEL"];
  const say = (text: string, ms?: number) =>
    caption(page, label === undefined ? text : `${label}: ${text}`, ms);
  const shots = process.env["INBOX_SHOTS"];
  const shot = async (name: string) => {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/${name}-${phone ? "375" : "desktop"}.png` });
    }
  };

  for (const [index, step] of STEPS.entries()) {
    await page.goto(`/w/${workspace}/inbox/primary`);
    await page.getByText(step.subject).first().waitFor({ timeout: 60_000 });
    await page.getByText(step.subject).first().click();
    await page.getByRole("heading", { name: step.subject }).waitFor();
    await page.locator("iframe").first().waitFor();
    await pause(700);
    const name = `${String(index + 1).padStart(2, "0")}-${step.subject.slice(0, 12).replaceAll(/\W+/gu, "-").toLowerCase()}`;
    await say(step.says, 1800);
    await shot(name);
    if (step.images === true) {
      await page
        .getByRole("button", { name: /Show images/u })
        .first()
        .click();
      await pause(1000);
      await say("Show images: they load now, and the frame fits them", 1800);
      await shot(`${name}-shown`);
    }
    // Read down through the message.
    const box = await page.locator("iframe").first().boundingBox();
    if (box !== null) {
      await page.mouse.move(box.x + box.width / 2, Math.max(box.y, 200) + 40);
    }
    for (let scroll = 0; scroll < (phone ? 4 : 2); scroll += 1) {
      await page.mouse.wheel(0, phone ? 420 : 360);
      await pause(650);
    }
    await shot(`${name}-end`);
  }
}
