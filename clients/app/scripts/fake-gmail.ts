// The Gmail API of the fake Google (scripts/fake-google.ts): a demo mailbox
// per fake Google account, served as the connectors module's Gmail adapter
// reads it (users.me only: profile, labels, messages list/get, history) and
// writes to it (messages modify, batchModify, trash and untrash, recorded in
// history).
// Development and recordings only. Every person, company and address here is
// fictional; domains are `.example`.
import { randomBytes } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";
import { crc32, deflateSync } from "node:zlib";

/** One stored message. */
interface StoredMessage {
  id: string;
  threadId: string;
  labelIds: string[];
  /** Epoch milliseconds. */
  internalDate: number;
  /** RFC 5322 bytes. */
  raw: string;
}

/** A message as history records name it. */
interface HistoryMessage {
  message: { id: string; threadId: string; labelIds: string[] };
  labelIds?: string[];
}

/** One history record (`users.history.list`). */
interface HistoryRecord {
  id: number;
  messagesAdded?: HistoryMessage[];
  labelsAdded?: HistoryMessage[];
  labelsRemoved?: HistoryMessage[];
}

export interface Mailbox {
  /** The account's address. */
  address: string;
  messages: StoredMessage[];
  history: HistoryRecord[];
  /** The history id before the first record still kept. */
  baseHistoryId: number;
  historyId: number;
}

const LABELS = [
  { id: "INBOX", name: "INBOX", type: "system" },
  { id: "SENT", name: "SENT", type: "system" },
  { id: "UNREAD", name: "UNREAD", type: "system" },
  { id: "STARRED", name: "STARRED", type: "system" },
  { id: "IMPORTANT", name: "IMPORTANT", type: "system" },
  { id: "SPAM", name: "SPAM", type: "system" },
  { id: "TRASH", name: "TRASH", type: "system" },
  { id: "CATEGORY_PERSONAL", name: "CATEGORY_PERSONAL", type: "system" },
  { id: "CATEGORY_UPDATES", name: "CATEGORY_UPDATES", type: "system" },
  { id: "CATEGORY_PROMOTIONS", name: "CATEGORY_PROMOTIONS", type: "system" },
  { id: "Label_1", name: "Clients", type: "user" },
];

const hex = (bytes: number) => randomBytes(bytes).toString("hex");

/** A demo attachment: a name and its bytes. */
interface DemoFile {
  name: string;
  type: string;
  content: string | Buffer;
}

/** A picture shown inside the HTML body (`cid:`), in a multipart/related. */
interface DemoInline extends DemoFile {
  /** Its Content-ID, without angle brackets. */
  cid: string;
}

/** A message of a demo mailbox, before it is laid out as RFC 5322. */
export interface DemoMessage {
  from: string;
  to?: string;
  cc?: string;
  subject: string;
  /** Hours before the mailbox was made. */
  hoursAgo: number;
  /** The text part; without one, the message is its HTML alone. */
  text?: string;
  html?: string;
  labels: string[];
  /** Thread key shared by the messages of one demo conversation. */
  thread?: string;
  /** The `thread` message this one answers. */
  replyTo?: string;
  headers?: Record<string, string>;
  files?: DemoFile[];
  inline?: DemoInline[];
}

/** A tiny but valid PDF, so attachments are real files. */
export function pdf(title: string): string {
  return [
    "%PDF-1.4",
    "1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj",
    "2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj",
    "3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj",
    `4 0 obj<</Length ${String(title.length + 30)}>>stream`,
    `BT /F1 14 Tf 24 72 Td (${title}) Tj ET`,
    "endstream endobj",
    "5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj",
    "trailer<</Root 1 0 R>>",
    "%%EOF",
  ].join("\n");
}

/** A PNG of `width` × `height` whose pixels `paint` gives as [r, g, b]. */
function png(
  width: number,
  height: number,
  paint: (x: number, y: number) => [number, number, number],
): Buffer {
  const chunk = (type: string, data: Buffer) => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([length, body, crc]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.writeUInt8(8, 8); // bit depth
  header.writeUInt8(2, 9); // truecolour
  const rows = Buffer.alloc(height * (1 + width * 3));
  for (let y = 0; y < height; y += 1) {
    const row = y * (1 + width * 3);
    for (let x = 0; x < width; x += 1) {
      const [r, g, b] = paint(x, y);
      rows.writeUInt8(r, row + 1 + x * 3);
      rows.writeUInt8(g, row + 2 + x * 3);
      rows.writeUInt8(b, row + 3 + x * 3);
    }
  }
  return Buffer.concat([
    Buffer.from("89504e470d0a1a0a", "hex"),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(rows)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/** A harbour at dusk: a sky gradient, a sun, hills and water. */
function harbourPhoto(): Buffer {
  const width = 640;
  const height = 420;
  return png(width, height, (x, y) => {
    const sun = Math.hypot(x - 450, y - 150) < 46;
    const hill = y > 250 + 40 * Math.sin(x / 70) - x / 16;
    const water = y > 330;
    if (water) {
      const ripple = Math.sin(x / 9 + y / 3) > 0.92 ? 40 : 0;
      return [24 + ripple, 52 + ripple, 74 + ripple];
    }
    if (hill) {
      return [30, 59, 51];
    }
    if (sun) {
      return [246, 196, 92];
    }
    const t = y / 330;
    return [Math.round(242 - 120 * t), Math.round(160 - 70 * t), Math.round(120 + 40 * t)];
  });
}

/** A small wordmark-like logo, sent inline. */
function brightloopLogo(): Buffer {
  return png(120, 32, (x, y) => {
    const ring = Math.abs(Math.hypot(x - 16, y - 16) - 10) < 3;
    const bar = x > 34 && x < 112 && y > 12 && y < 20;
    return ring || bar ? [216, 90, 48] : [255, 255, 255];
  });
}

/**
 * The demo mailbox, newest last. `images` is where the fake serves its demo
 * pictures, so "Show images" has something real to load.
 */
function demoMessages(me: string, images: string): DemoMessage[] {
  return [
    {
      from: "Maya Okafor <maya@northwind-studio.example>",
      to: me,
      cc: "Leo Brandt <leo@northwind-studio.example>",
      subject: "Q4 plan: can you review by Friday?",
      hoursAgo: 52,
      thread: "q4",
      labels: ["INBOX", "Label_1", "CATEGORY_PERSONAL"],
      text: "Hi Sam,\n\nHere is the first draft of the Q4 plan. Could you review it by Friday?\n\nMaya",
      html: `<p>Hi Sam,</p><p>Here is the first draft of the <b>Q4 plan</b>. Could you review it by Friday?</p><p>Maya</p>`,
      files: [
        { name: "q4-plan-draft.pdf", type: "application/pdf", content: pdf("Q4 plan, draft") },
      ],
    },
    {
      from: `Sam Rivera <${me}>`,
      to: "Maya Okafor <maya@northwind-studio.example>",
      subject: "Re: Q4 plan: can you review by Friday?",
      hoursAgo: 49,
      thread: "q4",
      replyTo: "q4",
      labels: ["SENT"],
      text: "Thanks Maya, I will look at it this week. Is the timeline fixed?\n\nSam",
    },
    {
      from: "The Weekly Ledger <hello@weekly-ledger.example>",
      to: me,
      subject: "Five tax dates before year end",
      hoursAgo: 30,
      labels: ["INBOX", "CATEGORY_PROMOTIONS"],
      headers: {
        "List-Unsubscribe": "<https://weekly-ledger.example/unsubscribe?u=demo>",
        "List-Id": "The Weekly Ledger <weekly.weekly-ledger.example>",
        Precedence: "bulk",
        // Gmail's own results: the Ledger signs its mail and passes DMARC, so
        // "Always show images from" can trust it (campbooks-cloud#200).
        "Authentication-Results":
          "mx.google.com; dkim=pass header.i=@weekly-ledger.example header.s=s1; " +
          "spf=pass smtp.mailfrom=bounce@weekly-ledger.example; " +
          "dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=weekly-ledger.example",
      },
      text: "Five tax dates before year end, plus what changes for freelancers in 2027.\n\nUnsubscribe: https://weekly-ledger.example/unsubscribe",
      html: `<style>body{background:url(${images}/track.png?css=1)} .hero{color:#0b6}</style>
<script>document.title = "stolen"</script>
<table width="100%" style="font-family:Georgia,serif;max-width:560px;margin:0 auto">
<tr><td><img src="${images}/ledger-banner.svg" alt="The Weekly Ledger" width="560" height="120" style="display:block;width:100%;height:auto"></td></tr>
<tr><td><h1 class="hero" style="font-size:24px;margin:16px 0">Five tax dates before year end</h1>
<p style="font-size:16px;line-height:1.5">Mark these in your calendar, and read <a href="https://weekly-ledger.example/articles/tax-dates" onclick="steal()">what changes for freelancers in 2027</a>.</p>
<ol style="font-size:16px;line-height:1.6"><li>15 November: quarterly VAT return</li><li>30 November: second payment on account</li><li>15 December: payroll year-end checks</li><li>20 December: invoices for the year</li><li>31 December: pension contributions</li></ol>
<p style="color:#666;font-size:13px">You get this because you subscribed. <a href="https://weekly-ledger.example/unsubscribe?u=demo">Unsubscribe</a>.</p>
<img src="${images}/track.png?u=demo" width="1" height="1" alt="" style="display:none">
<form action="https://weekly-ledger.example/login"><input name="password"></form>
</td></tr></table>`,
    },
    {
      // Pretends to be the Ledger and fails DMARC: trusting the Ledger never
      // shows its images (campbooks-cloud#200).
      from: "The Weekly Ledger <hello@weekly-ledger.example>",
      to: me,
      subject: "Your Ledger subscription could not be renewed",
      hoursAgo: 28,
      labels: ["INBOX", "CATEGORY_PROMOTIONS"],
      headers: {
        "Authentication-Results":
          "mx.google.com; spf=pass smtp.mailfrom=bounce@ledger-renewals.example; " +
          "dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=weekly-ledger.example",
      },
      text: "Your subscription could not be renewed. Update your card to keep reading.",
      html: `<div style="font-family:Georgia,serif;max-width:560px"><img src="${images}/ledger-banner.svg" alt="The Weekly Ledger" width="560" height="120" style="width:100%;height:auto"><p>Your subscription could not be renewed. Update your card to keep reading.</p></div>`,
    },
    {
      from: "Cloudhost Billing <billing@cloudhost.example>",
      to: me,
      subject: "Your invoice CH-20418 for €148.00",
      hoursAgo: 26,
      labels: ["INBOX", "CATEGORY_UPDATES"],
      text: "Your invoice CH-20418 for €148.00 is due on 16 October. Thank you for your payment.",
      html: `<div style="font-family:Helvetica,Arial,sans-serif"><p>Your invoice <b>CH-20418</b> for <b>€148.00</b> is due on 16 October.</p><p>The invoice is attached as a PDF.</p></div>`,
      files: [{ name: "CH-20418.pdf", type: "application/pdf", content: pdf("Invoice CH-20418") }],
    },
    {
      // HTML alone, opening with a head and never saying <body> (some mailers
      // send it): search finds it by a word only its text holds,
      // "harbourlight" (campbooks-cloud#218; the search-head-html recording).
      from: "Quay Office <office@quay-moorings.example>",
      to: me,
      subject: "Your berth for next season",
      hoursAgo: 24,
      labels: ["INBOX", "CATEGORY_UPDATES"],
      html: `<head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Quay Office</title><style>p{font-family:Georgia,serif}</style><p>Hello Sam,</p><p>Your berth at the harbourlight pontoon is kept for next season. Nothing to do until March.</p><p>Quay Office</p>`,
    },
    {
      // HTML alone and long (over 256 KiB): read in time, but a reply or
      // forward quotes only its first part, and says so
      // (campbooks-cloud#255; the quote-long-html recording). Its words never say
      // "harbourlight": search-head-html finds the berth email by that word alone.
      from: "Quay Office <office@quay-moorings.example>",
      to: me,
      subject: "Tide tables for the season",
      hoursAgo: 23,
      labels: ["INBOX", "CATEGORY_UPDATES"],
      html: `<div style="font-family:Georgia,serif"><p>Hello Sam,</p><p>Here are the tide tables at the quay for the whole season.</p><table>${Array.from(
        { length: 3_000 },
        (_, day) =>
          `<tr><td>Day ${String(day + 1)}</td><td>High water 06:12 and 18:40</td><td>Low water 00:05 and 12:27</td></tr>`,
      ).join("")}</table><p>Quay Office</p></div>`,
    },
    {
      from: "Accounts <no-reply@accounts.example>",
      to: me,
      subject: "New sign-in to your account from Linux",
      hoursAgo: 20,
      labels: ["INBOX", "UNREAD", "CATEGORY_UPDATES"],
      headers: { "Auto-Submitted": "auto-generated" },
      text: "We noticed a new sign-in to your account from Linux. If this was you, there is nothing to do.",
    },
    {
      from: "Tiago Mendes <tiago@brightloop.example>",
      to: me,
      subject: "Move Thursday's call?",
      hoursAgo: 14,
      labels: ["INBOX", "UNREAD", "IMPORTANT", "CATEGORY_PERSONAL"],
      text: "Hi Sam,\n\nSomething came up on Thursday. Could we do Friday at 11:00 instead? Same link.\n\nTiago",
    },
    {
      from: "Design Notes <digest@design-notes.example>",
      to: me,
      subject: "This week: calmer dashboards",
      hoursAgo: 10,
      labels: ["INBOX", "UNREAD", "CATEGORY_PROMOTIONS"],
      headers: { "List-Unsubscribe": "<mailto:leave@design-notes.example>", Precedence: "list" },
      text: "Three essays on calmer dashboards, and a webinar on 20 October.",
      html: `<div style="font-family:system-ui,sans-serif;max-width:520px"><img src="${images}/notes-cover.svg" alt="Design Notes" width="520" height="160" style="width:100%;height:auto;border-radius:8px"><h2>Calmer dashboards</h2><p>Three essays on calmer dashboards, and a <a href="https://design-notes.example/webinar">webinar on 20 October</a>.</p></div>`,
    },
    {
      from: "IT Support <it-support@at3lier-sol.example>",
      to: me,
      subject: "Action required: verify your mailbox",
      hoursAgo: 8,
      labels: ["INBOX", "UNREAD", "CATEGORY_PERSONAL"],
      text: "Dear user,\n\nYour mailbox will be closed today unless you verify your account using the link below.\n\nhttps://verify-mailbox.example/login\n\nIT Support",
    },
    {
      from: "Pedro Alves <pedro.alves@example.com>",
      to: me,
      subject: "Lease for the studio",
      hoursAgo: 7,
      labels: ["INBOX", "UNREAD", "CATEGORY_PERSONAL"],
      text: "Hello Sam,\n\nPlease sign the attached lease and send it back by Friday.\n\nBest,\nPedro",
      files: [{ name: "lease-rua-do-sol-12.pdf", type: "application/pdf", content: pdf("Lease") }],
    },
    {
      from: "Marta Ferreira <marta@casa-lume.example>",
      to: me,
      subject: "Walkthrough on Thursday",
      hoursAgo: 5,
      labels: ["INBOX", "UNREAD", "CATEGORY_PERSONAL"],
      text: "Hi Sam,\n\nHope the move went well. Could you confirm Thursday's walkthrough? The builders need to know by tomorrow.\n\nMarta",
    },
    {
      from: "Joana Reis <joana@atelier-norte.example>",
      to: me,
      subject: "NDA before Friday's kickoff",
      hoursAgo: 6,
      labels: ["INBOX", "UNREAD", "STARRED", "CATEGORY_PERSONAL"],
      text: "Hi Sam,\n\nCould you send the NDA back signed before Friday? We cannot share the brand files until then.\n\nThanks,\nJoana",
      files: [{ name: "atelier-norte-nda.pdf", type: "application/pdf", content: pdf("NDA") }],
    },
    {
      from: "Maya Okafor <maya@northwind-studio.example>",
      to: me,
      subject: "Re: Q4 plan: can you review by Friday?",
      hoursAgo: 3,
      thread: "q4",
      replyTo: "q4",
      labels: ["INBOX", "UNREAD", "Label_1", "CATEGORY_PERSONAL"],
      text: "The timeline can move by two weeks; the price stays the same. Attached is version 3.\n\nMaya",
      html: `<p>The timeline can move by two weeks; the price stays the same.</p><p>Attached is <b>version 3</b>.</p><p><img src="${images}/northwind-logo.svg" alt="Northwind Studio" width="140" height="40"></p>`,
      files: [
        { name: "q4-plan-v3.pdf", type: "application/pdf", content: pdf("Q4 plan, version 3") },
      ],
    },
    {
      from: "Ana Lopes <ana@brightloop.example>",
      to: me,
      subject: "Photos and tickets for the harbour walk",
      hoursAgo: 1,
      labels: ["INBOX", "UNREAD", "CATEGORY_PERSONAL"],
      text: "Hi Sam,\n\nHere is the photo from Saturday, the tickets for the boat and the itinerary. The route map is on the walking site.\n\nAna",
      html: `<p><img src="cid:logo@brightloop.example" alt="Brightloop" width="120" height="32"></p>
<p>Hi Sam,</p><p>Here is the photo from Saturday, the tickets for the boat and the itinerary.</p>
<p>The route map, from the walking site:</p>
<p><img src="${images}/route-map.svg" alt="Route map" width="480" height="160" style="width:100%;max-width:480px;height:auto"></p>
<p>Ana</p>`,
      inline: [
        {
          cid: "logo@brightloop.example",
          name: "brightloop-logo.png",
          type: "image/png",
          content: brightloopLogo(),
        },
      ],
      files: [
        { name: "harbour-at-dusk.png", type: "image/png", content: harbourPhoto() },
        {
          name: "boat-tickets.pdf",
          type: "application/pdf",
          content: pdf("Boat tickets, 2 adults"),
        },
        {
          name: "itinerary.html",
          type: "text/html",
          content: `<html><body><h1>Itinerary</h1><script>alert(document.cookie)</script></body></html>`,
        },
      ],
    },
  ];
}

const CRLF = "\r\n";

function wrap64(text: string | Buffer): string {
  return (
    (typeof text === "string" ? Buffer.from(text) : text)
      .toString("base64")
      .match(/.{1,76}/gu)
      ?.join(CRLF) ?? ""
  );
}

/** Lay out `message` as RFC 5322 (multipart when it has HTML or files). */
export function rfc822(
  message: DemoMessage,
  date: Date,
  messageId: string,
  parent?: string,
): string {
  const headers: [string, string][] = [
    ["From", message.from],
    ["To", message.to ?? ""],
    ...(message.cc === undefined ? [] : [["Cc", message.cc] as [string, string]]),
    ["Subject", message.subject],
    ["Date", date.toUTCString().replace("GMT", "+0000")],
    ["Message-ID", messageId],
    ...(parent === undefined
      ? []
      : [["In-Reply-To", parent] as [string, string], ["References", parent] as [string, string]]),
    ["MIME-Version", "1.0"],
    ...Object.entries(message.headers ?? {}),
  ];
  const textPart = [
    "Content-Type: text/plain; charset=utf-8",
    "Content-Transfer-Encoding: base64",
    "",
    wrap64(message.text ?? ""),
  ].join(CRLF);
  const alt = `alt-${hex(6)}`;
  const alternative = [
    `--${alt}`,
    textPart,
    `--${alt}`,
    "Content-Type: text/html; charset=utf-8",
    "Content-Transfer-Encoding: base64",
    "",
    wrap64(message.html ?? ""),
    `--${alt}--`,
  ].join(CRLF);
  const files = message.files ?? [];
  const inline = message.inline ?? [];
  // The body: text, text and HTML, or text and HTML with the pictures the
  // HTML shows (`cid:`) beside it in a multipart/related.
  const related = `rel-${hex(6)}`;
  const relatedBody = [
    `--${related}`,
    [`Content-Type: multipart/alternative; boundary="${alt}"`, "", alternative].join(CRLF),
    ...inline.flatMap((part) => [
      `--${related}`,
      `Content-Type: ${part.type}; name="${part.name}"`,
      `Content-Disposition: inline; filename="${part.name}"`,
      `Content-ID: <${part.cid}>`,
      "Content-Transfer-Encoding: base64",
      "",
      wrap64(part.content),
    ]),
    `--${related}--`,
  ].join(CRLF);
  let type: string;
  let body: string;
  if (message.text === undefined && files.length === 0 && inline.length === 0) {
    // HTML alone, as some mailers send it.
    type = "text/html; charset=utf-8";
    headers.push(["Content-Transfer-Encoding", "base64"]);
    body = wrap64(message.html ?? "");
  } else if (files.length === 0 && message.html === undefined) {
    type = "text/plain; charset=utf-8";
    headers.push(["Content-Transfer-Encoding", "base64"]);
    body = wrap64(message.text ?? "");
  } else if (files.length === 0 && inline.length === 0) {
    type = `multipart/alternative; boundary="${alt}"`;
    body = alternative;
  } else if (files.length === 0) {
    type = `multipart/related; boundary="${related}"`;
    body = relatedBody;
  } else {
    const mixed = `mixed-${hex(6)}`;
    type = `multipart/mixed; boundary="${mixed}"`;
    let first: string;
    if (message.html === undefined) {
      first = textPart;
    } else if (inline.length === 0) {
      first = [`Content-Type: multipart/alternative; boundary="${alt}"`, "", alternative].join(
        CRLF,
      );
    } else {
      first = [`Content-Type: multipart/related; boundary="${related}"`, "", relatedBody].join(
        CRLF,
      );
    }
    body = [
      `--${mixed}`,
      first,
      ...files.flatMap((file) => [
        `--${mixed}`,
        `Content-Type: ${file.type}; name="${file.name}"`,
        `Content-Disposition: attachment; filename="${file.name}"`,
        "Content-Transfer-Encoding: base64",
        "",
        wrap64(file.content),
      ]),
      `--${mixed}--`,
    ].join(CRLF);
  }
  headers.push(["Content-Type", type]);
  return `${headers.map(([name, value]) => `${name}: ${value}`).join(CRLF)}${CRLF}${CRLF}${body}${CRLF}`;
}

/** Store `raw` in `mailbox` as a new message and record it in history. */
function appendMessage(
  mailbox: Mailbox,
  raw: string,
  labelIds: string[],
  options: { threadId?: string; internalDate?: number } = {},
): StoredMessage {
  const message: StoredMessage = {
    id: hex(8),
    threadId: options.threadId ?? hex(8),
    labelIds,
    internalDate: options.internalDate ?? Date.now(),
    raw,
  };
  mailbox.messages.push(message);
  mailbox.historyId += 1;
  mailbox.history.push({
    id: mailbox.historyId,
    messagesAdded: [{ message: { id: message.id, threadId: message.threadId, labelIds } }],
  });
  return message;
}

/**
 * `count` more issues of the Design Notes digest, a few hours apart (so
 * hundreds fit in the window a first sync reads): mail that looks the same,
 * for teaching once (`--lookalikes` of the fake provider).
 */
function lookalikes(me: string, count: number): DemoMessage[] {
  const topics = ["typography", "colour", "forms", "motion", "tables", "empty states"];
  return Array.from({ length: count }, (_, index) => ({
    from: "Design Notes <digest@design-notes.example>",
    to: me,
    subject: `This week: ${topics[index % topics.length] ?? "layout"} (issue ${String(count + 120 - index)})`,
    hoursAgo: 34 + index * 3,
    // Unread, as a digest nobody opens: what Scout notices.
    labels: ["INBOX", "UNREAD", "CATEGORY_PROMOTIONS"],
    headers: { "List-Unsubscribe": "<mailto:leave@design-notes.example>", Precedence: "list" },
    text: "Three short essays, and what we read this week.",
  }));
}

/**
 * The sanitiser's fixture emails (`crates/modules/conversations/fixtures/
 * email-html`, campbooks-cloud#160) as demo messages, newest first: real-
 * world-shaped HTML for showing the reader. Their fictional image hosts are
 * pointed at the fake's own pictures, so "Show images" loads something.
 */
export function htmlMailDemos(
  me: string,
  images: string,
  fixtures: Record<string, string>,
): DemoMessage[] {
  const pictures = (html: string) =>
    html.replaceAll(/https:\/\/(?:images|cdn)\.example\//gu, `${images}/fixture/`);
  const demos: [string, string, string][] = [
    ["newsletter", "HTMLemail <news@news.example>", "A simple call to action"],
    ["receipt", "Harbourline <receipts@shop.example>", "Your Harbourline receipt R-20418"],
    ["marketing-600", "Harbourline <sale@shop.example>", "Thirty per cent off waterproofs"],
    ["marketing-fluid", "Harbourline <news@shop.example>", "New in: the autumn range"],
    ["outlook", "Ana Lopes <ana@brightloop.example>", "Re: Harbour walk figures"],
    ["gmail", "Sam Rivera <sam.rivera@rivera.example>", "Re: Second instalment"],
    ["hostile", "Prize Desk <win@prize-desk.example>", "You have won (a hostile test message)"],
  ];
  return demos.flatMap(([name, from, subject], index) => {
    const html = fixtures[name];
    return html === undefined
      ? []
      : [
          {
            from,
            to: me,
            subject,
            hoursAgo: 0.1 + index * 0.05,
            labels: ["INBOX", "UNREAD", "CATEGORY_PERSONAL"],
            text: subject,
            html: pictures(html),
          },
        ];
  });
}

/** A new demo mailbox for `address`, with `more` messages on top of the demo. */
export function createMailbox(
  address: string,
  images: string,
  extra = 0,
  more: DemoMessage[] = [],
): Mailbox {
  const mailbox: Mailbox = {
    address,
    messages: [],
    history: [],
    baseHistoryId: 1000,
    historyId: 1000,
  };
  const threads = new Map<string, { threadId: string; messageId: string }>();
  const now = Date.now();
  for (const demo of [...demoMessages(address, images), ...lookalikes(address, extra), ...more]) {
    const date = new Date(now - demo.hoursAgo * 3_600_000);
    const messageId = `<${hex(6)}@demo.example>`;
    const parent = demo.replyTo === undefined ? undefined : threads.get(demo.replyTo);
    const raw = rfc822(demo, date, messageId, parent?.messageId);
    const stored = appendMessage(mailbox, raw, demo.labels, {
      ...(parent === undefined ? {} : { threadId: parent.threadId }),
      internalDate: date.getTime(),
    });
    if (demo.thread !== undefined && !threads.has(demo.thread)) {
      threads.set(demo.thread, { threadId: stored.threadId, messageId });
    }
  }
  // The backfill reads the mailbox as it is; history starts after it.
  mailbox.history = [];
  mailbox.baseHistoryId = mailbox.historyId;
  return mailbox;
}

/** A message that arrives while the app is open (`POST /fake/deliver`). */
export function deliverDemo(mailbox: Mailbox): StoredMessage {
  const date = new Date();
  const raw = rfc822(
    {
      from: "Rui Matos <rui@contabilidade-rm.example>",
      to: mailbox.address,
      subject: "VAT Q3: two receipts missing",
      hoursAgo: 0,
      labels: [],
      text: "Hi Sam,\n\nI still need the two August receipts from Papelaria Sol. Friday at the latest, please.\n\nRui",
    },
    date,
    `<${hex(6)}@demo.example>`,
  );
  return appendMessage(mailbox, raw, ["INBOX", "UNREAD", "CATEGORY_PERSONAL"], {
    internalDate: date.getTime(),
  });
}

function json(response: ServerResponse, status: number, value: unknown) {
  response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  response.end(JSON.stringify(value));
}

const meta = (message: StoredMessage) => ({
  id: message.id,
  threadId: message.threadId,
  labelIds: message.labelIds,
  internalDate: String(message.internalDate),
});

/** Change a message's labels as Gmail does: only real changes reach history. */
function modifyLabels(mailbox: Mailbox, message: StoredMessage, add: string[], remove: string[]) {
  const added = add.filter((label) => !message.labelIds.includes(label));
  const removed = remove.filter(
    (label) => message.labelIds.includes(label) && !add.includes(label),
  );
  if (added.length === 0 && removed.length === 0) {
    return;
  }
  message.labelIds = [...message.labelIds, ...added].filter((label) => !removed.includes(label));
  const ref = { id: message.id, threadId: message.threadId, labelIds: message.labelIds };
  mailbox.historyId += 1;
  mailbox.history.push({
    id: mailbox.historyId,
    ...(added.length > 0 ? { labelsAdded: [{ message: ref, labelIds: added }] } : {}),
    ...(removed.length > 0 ? { labelsRemoved: [{ message: ref, labelIds: removed }] } : {}),
  });
  console.log(`fake Gmail: ${message.id} +[${added.join(",")}] -[${removed.join(",")}]`);
}

const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

/**
 * Answer a Gmail write (`POST …/messages/{id}/modify`, `…/messages/batchModify`,
 * `…/messages/{id}/trash`, `…/messages/{id}/untrash`) for `mailbox`. Trash adds
 * `TRASH` (and takes the message out of spam), keeping its other labels, so
 * untrash, which removes `TRASH`, puts it back where it was. Returns false when
 * the path is not one this fake serves.
 */
export async function writeGmail(
  request: IncomingMessage,
  url: URL,
  response: ServerResponse,
  mailbox: Mailbox,
): Promise<boolean> {
  const path = url.pathname.replace(/^\/gmail\/v1\/users\/me\//u, "");
  if (request.method !== "POST") {
    return false;
  }
  let text = "";
  for await (const chunk of request) text += String(chunk);
  const body = (text === "" ? {} : JSON.parse(text)) as Record<string, unknown>;
  const add = strings(body["addLabelIds"]);
  const remove = strings(body["removeLabelIds"]);
  if (path === "messages/batchModify") {
    for (const id of strings(body["ids"])) {
      const message = mailbox.messages.find((candidate) => candidate.id === id);
      if (message !== undefined) {
        modifyLabels(mailbox, message, add, remove);
      }
    }
    response.writeHead(204);
    response.end();
    return true;
  }
  const [, id, verb] = /^messages\/(\w+)\/(modify|trash|untrash)$/u.exec(path) ?? [];
  if (id === undefined) {
    return false;
  }
  const message = mailbox.messages.find((candidate) => candidate.id === id);
  if (message === undefined) {
    json(response, 404, { error: { code: 404, message: "Not Found" } });
    return true;
  }
  if (verb === "trash") {
    modifyLabels(mailbox, message, ["TRASH"], ["SPAM"]);
  } else if (verb === "untrash") {
    modifyLabels(mailbox, message, [], ["TRASH"]);
  } else {
    modifyLabels(mailbox, message, add, remove);
  }
  json(response, 200, meta(message));
  return true;
}

/** What `mailbox` holds, newest first: each message's subject and labels. */
export function describeMailbox(mailbox: Mailbox): { subject: string; labels: string[] }[] {
  return mailbox.messages
    .toSorted((a, b) => b.internalDate - a.internalDate)
    .map((message) => ({
      subject: /^Subject: (.*)$/mu.exec(message.raw)?.[1]?.trim() ?? "",
      labels: message.labelIds.toSorted(),
    }));
}

/**
 * Answer a Gmail API request (`/gmail/v1/users/me/…`) for `mailbox`.
 * Returns false when the path is not one this fake serves.
 */
export function handleGmail(
  request: IncomingMessage,
  url: URL,
  response: ServerResponse,
  mailbox: Mailbox,
): boolean {
  const path = url.pathname.replace(/^\/gmail\/v1\/users\/me\//u, "");
  if (request.method !== "GET") {
    return false;
  }
  if (path === "profile") {
    json(response, 200, {
      emailAddress: mailbox.address,
      messagesTotal: mailbox.messages.length,
      historyId: String(mailbox.historyId),
    });
    return true;
  }
  if (path === "labels") {
    json(response, 200, { labels: LABELS });
    return true;
  }
  if (path === "messages") {
    const query = url.searchParams.get("q") ?? "";
    const after = /after:(\d+)/u.exec(query)?.[1];
    // `rfc822msgid:<id>`: how a sender checks whether a send it lost track of went out.
    const wanted = /rfc822msgid:(\S+)/u.exec(query)?.[1]?.toLowerCase();
    const max = Number(url.searchParams.get("maxResults") ?? "100");
    const offset = Number(url.searchParams.get("pageToken") ?? "0");
    const matching = mailbox.messages
      .filter((message) => after === undefined || message.internalDate > Number(after) * 1000)
      .filter(
        (message) =>
          wanted === undefined || message.raw.toLowerCase().includes(`message-id: <${wanted}>`),
      )
      .toSorted((a, b) => b.internalDate - a.internalDate);
    const page = matching.slice(offset, offset + max);
    json(response, 200, {
      messages: page.map((message) => ({ id: message.id, threadId: message.threadId })),
      ...(offset + max < matching.length ? { nextPageToken: String(offset + max) } : {}),
      resultSizeEstimate: matching.length,
    });
    return true;
  }
  const one = /^messages\/(\w+)$/u.exec(path)?.[1];
  if (one !== undefined) {
    const message = mailbox.messages.find((candidate) => candidate.id === one);
    if (message === undefined) {
      json(response, 404, { error: { code: 404, message: "Not Found" } });
    } else if (url.searchParams.get("format") === "raw") {
      json(response, 200, {
        ...meta(message),
        raw: Buffer.from(message.raw).toString("base64url"),
      });
    } else {
      json(response, 200, meta(message));
    }
    return true;
  }
  if (path === "history") {
    const start = Number(url.searchParams.get("startHistoryId") ?? "0");
    if (start < mailbox.baseHistoryId) {
      json(response, 404, { error: { code: 404, message: "Requested entity was not found." } });
      return true;
    }
    json(response, 200, {
      history: mailbox.history
        .filter((record) => record.id > start)
        .map((record) => ({ ...record, id: String(record.id) })),
      historyId: String(mailbox.historyId),
    });
    return true;
  }
  return false;
}

/**
 * `users.messages.send` as a multipart upload (`uploadType=multipart`): a
 * JSON part with the `threadId`, then the RFC 5322 message. The message is
 * stored as SENT in that thread (or a new one) and recorded in history, as
 * Gmail does; nothing is delivered anywhere.
 */
export function sendMessage(
  mailbox: Mailbox,
  contentType: string,
  body: string,
): { status: number; value: unknown } {
  const boundary = /boundary="?([^";]+)"?/u.exec(contentType)?.[1];
  if (boundary === undefined) {
    return { status: 400, value: { error: { code: 400, message: "Not a multipart upload" } } };
  }
  const parts = body.split(`--${boundary}`).slice(1, -1);
  const content = (part: string) => part.slice(part.indexOf("\r\n\r\n") + 4).replace(/\r\n$/u, "");
  const [metadataPart, messagePart] = parts;
  if (metadataPart === undefined || messagePart === undefined) {
    return { status: 400, value: { error: { code: 400, message: "Missing parts" } } };
  }
  const metadata = JSON.parse(content(metadataPart) || "{}") as { threadId?: string };
  const known = mailbox.messages.some((message) => message.threadId === metadata.threadId);
  const sent = appendMessage(mailbox, content(messagePart), ["SENT"], {
    ...(metadata.threadId !== undefined && known ? { threadId: metadata.threadId } : {}),
  });
  console.log(`fake Google: sent a message in thread ${sent.threadId} (not delivered)`);
  return { status: 200, value: { id: sent.id, threadId: sent.threadId, labelIds: sent.labelIds } };
}

/** The demo pictures the newsletters link to (served under `/images/`). */
export function demoImage(name: string): { type: string; body: string } | undefined {
  const svg = (width: number, height: number, fill: string, label: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="${String(width)}" height="${String(height)}" viewBox="0 0 ${String(width)} ${String(height)}"><rect width="100%" height="100%" rx="8" fill="${fill}"/><text x="50%" y="55%" text-anchor="middle" font-family="Georgia,serif" font-size="${String(Math.round(height / 4))}" fill="#fff">${label}</text></svg>`;
  if (name.startsWith("fixture/")) {
    // A fixture email's picture: a labelled panel, at the size the email gives it.
    return { type: "image/svg+xml", body: svg(600, 300, "#5b7f95", name.slice(8)) };
  }
  switch (name) {
    case "ledger-banner.svg": {
      return { type: "image/svg+xml", body: svg(560, 120, "#2f5d50", "The Weekly Ledger") };
    }
    case "notes-cover.svg": {
      return { type: "image/svg+xml", body: svg(520, 160, "#6b4f8a", "Design Notes") };
    }
    case "northwind-logo.svg": {
      return { type: "image/svg+xml", body: svg(140, 40, "#3d3d3d", "Northwind") };
    }
    case "route-map.svg": {
      return { type: "image/svg+xml", body: svg(480, 160, "#3a6f8f", "Harbour walk, 6 km") };
    }
    case "track.png": {
      // A 1×1 transparent PNG: what a tracking pixel looks like.
      return {
        type: "image/png",
        body: Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=",
          "base64",
        ).toString("latin1"),
      };
    }
    default: {
      return undefined;
    }
  }
}
