import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout's reply streams (campbooks-cloud#288): its thinking shows as it
// arrives, then the reply types into the draft. Needs only the Vite dev
// server: the page's /bff calls are answered in the page from the demo data
// the screen stories use (`test/demo/*`), and the stream route sends its
// frames a few at a time, as the server does. The server's side (limits,
// checks, the model call) is the conversations module's tests.
//
//   pnpm --dir clients/app exec vite --port 5288
//   pnpm --dir clients record reply-stream --base-url http://localhost:5288
//
// Fictional person and mail only. Nothing is sent: the dialog closes with
// Esc at the end.

/**
 * The page's stand-in for the BFF, installed before the app loads. A script
 * string: the recordings are type-checked without the DOM types.
 */
const FAKE_BFF = `(() => {
  const original = window.fetch.bind(window);
  const demo = Promise.all([
    import("/test/demo/attention.ts"),
    import("/test/demo/conversations.ts"),
    import("/test/demo/workspaces.ts"),
  ]);
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const frame = (event, data) => "event: " + event + "\\ndata: " + JSON.stringify(data) + "\\n\\n";
  const json = (body, status = 200) => Response.json(body, { status });
  let drafts = 0;
  const THINKING = [
    [
      "Marta asks whether Thursday works for the walkthrough of the studio. ",
      "Sam has written to her before, informally, first names, signs off with Best. ",
      "Option 1 is a plain yes. ",
      "Keep it short and warm, and confirm the time she proposed.",
    ],
    [
      "The member wrote: 1 and 3, warmer. ",
      "Suggestion 1 says yes to Thursday; suggestion 3 promises to confirm by tomorrow. ",
      "Combine them: a yes in principle, with the confirmation coming tomorrow. ",
      "Warmer than the last draft: thank her, and say it is good to hear from her.",
    ],
  ];
  const REPLIES = [
    "Hi Marta,\\n\\nThursday works for me. See you at the walkthrough at 10:00.\\n\\nBest,\\nSam",
    "Hi Marta,\\n\\nThank you, it's lovely to hear from you! Thursday should work for me. " +
      "I'll confirm by tomorrow once I've checked the morning.\\n\\nBest,\\nSam",
  ];
  function stream(messageId) {
    const turn = Math.min(drafts, THINKING.length - 1);
    drafts += 1;
    const encoder = new TextEncoder();
    return new Response(
      new ReadableStream({
        async start(controller) {
          const send = (event, data) => controller.enqueue(encoder.encode(frame(event, data)));
          await sleep(500);
          for (const line of THINKING[turn]) {
            for (const word of line.split(/(?<= )/u)) {
              send("thinking", { delta: word });
              await sleep(55);
            }
            await sleep(250);
          }
          const reply = REPLIES[turn];
          for (const word of reply.split(/(?<=[ \\n])/u)) {
            send("text", { delta: word });
            await sleep(70);
          }
          send("done", { message_id: messageId, text: reply });
          controller.close();
        },
      }),
      { headers: { "content-type": "text/event-stream" } },
    );
  }
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url, location.href);
    if (!url.pathname.startsWith("/bff/")) return original(input, init);
    const [attention, conversations, workspaces] = await demo;
    const { CLEARED, COUNTS, ITEMS, MARTA, MARTA_DETAIL, NEEDS, SOFIA } = attention;
    const { INBOX, STREAM_COUNTS, W } = conversations;
    const { DUARTE, SESSION } = workspaces;
    const answered = MARTA_DETAIL.messages.at(-1).id;
    const route = request.method + " " + url.pathname;
    await sleep(80);
    switch (route) {
      case "GET /bff/session":
        return json(SESSION);
      case "GET /bff/api/v1/workspaces":
        return json({ workspaces: [DUARTE] });
      case "GET " + W + "/inboxes":
        return json({ inboxes: [INBOX] });
      case "GET " + W + "/streams":
        return json({ streams: STREAM_COUNTS });
      case "GET " + W + "/attention":
        return url.searchParams.get("state") === "cleared"
          ? json({ items: CLEARED, counts: [] })
          : json({ items: ITEMS, counts: COUNTS });
      case "GET " + W + "/conversations":
        return json({ conversations: [...NEEDS, SOFIA], next_cursor: null });
      case "GET " + W + "/conversations/" + MARTA.id:
        return json(MARTA_DETAIL);
      case "POST " + W + "/conversations/" + MARTA.id + "/reply/suggestions":
        await sleep(700);
        return json({
          message_id: answered,
          intents: [
            "Yes, Thursday works",
            "Ask to move it to the afternoon",
            "Say you'll confirm by tomorrow",
          ],
        });
      case "POST " + W + "/conversations/" + MARTA.id + "/reply/draft/stream":
        return stream(answered);
      default:
        return json({ status: 404, code: "not_found", title: "Not found" }, 404);
    }
  };
})()`;

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function replyStream({ page }: ScenarioContext): Promise<void> {
  await page.addInitScript(FAKE_BFF);
  const phone = isPhone(page);
  // The demo workspace's id, as `test/demo/workspaces.ts` has it.
  const workspace = await page
    .goto("/")
    .then(() => page.evaluate(`import("/test/demo/workspaces.ts").then((demo) => demo.DUARTE.id)`));
  await page.goto(`/w/${String(workspace)}`);
  const marta = page.getByText("Marta Ferreira is waiting for your reply").first();
  await marta.waitFor();
  await caption(page, "Demo workspace, fictional mail; Scout's answers are scripted", 1800);

  await marta.click();
  await (phone
    ? page
        .getByRole("button", { name: /^Reply/u })
        .first()
        .click()
    : page.keyboard.press("r"));
  const dialog = page.getByRole("dialog", { name: /^Reply to/u });
  const yes = dialog.getByRole("button", { name: /Yes, Thursday works/u });
  await yes.waitFor();
  await caption(page, "Pick a likely reply", 1400);
  await (phone ? yes.click() : page.keyboard.press("1"));

  // The thinking arrives first, then the words type into the draft.
  await dialog.getByText("Scout is thinking…").waitFor();
  await caption(page, "Scout's thinking shows as it arrives", 2600);
  const text = dialog.getByRole("textbox", { name: "Your reply" });
  await text.waitFor();
  await caption(page, "Then the reply types itself in", 1600);
  await dialog.getByRole("button", { name: /^Send/u }).and(page.locator(":enabled")).waitFor();
  await caption(page, "The thinking folds away once the reply is written", 1800);
  await dialog.getByText("How Scout thought about it").click();
  await pause(1800);

  // Back to the choices, and combine two of them in plain words.
  await dialog.getByRole("button", { name: "Back to the choices" }).click();
  await dialog.getByRole("textbox", { name: "Or say it in your words" }).fill("1 and 3, warmer");
  await caption(page, "Say “1 and 3, warmer”: Scout gets the suggestions with your words", 2200);
  await dialog.getByRole("button", { name: "Draft it" }).click();
  await dialog.getByText("Scout is thinking…").waitFor();
  await pause(2600);
  await dialog.getByRole("button", { name: /^Send/u }).and(page.locator(":enabled")).waitFor();
  await caption(page, "One reply, from both suggestions", 2200);
  await page.keyboard.press("Escape");
  await pause(900);
}
