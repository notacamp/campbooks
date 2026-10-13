// The Gmail API of the fake Google (scripts/fake-google.ts): a demo mailbox
// per fake Google account, served as the connectors module's Gmail adapter
// reads it (users.me only: profile, labels, messages list/get, history).
// Development and recordings only. Every person, company and address here is
// fictional; domains are `.example`.
import { randomBytes } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";

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

/** One history record (`users.history.list`). */
interface HistoryRecord {
  id: number;
  messagesAdded?: { message: { id: string; threadId: string; labelIds: string[] } }[];
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
  content: string;
}

/** A message of the demo mailbox, before it is laid out as RFC 5322. */
interface DemoMessage {
  from: string;
  to?: string;
  cc?: string;
  subject: string;
  /** Hours before the mailbox was made. */
  hoursAgo: number;
  text: string;
  html?: string;
  labels: string[];
  /** Thread key shared by the messages of one demo conversation. */
  thread?: string;
  /** The `thread` message this one answers. */
  replyTo?: string;
  headers?: Record<string, string>;
  files?: DemoFile[];
}

/** A tiny but valid PDF, so attachments are real files. */
function pdf(title: string): string {
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
  ];
}

const CRLF = "\r\n";

function wrap64(text: string): string {
  return (
    Buffer.from(text)
      .toString("base64")
      .match(/.{1,76}/gu)
      ?.join(CRLF) ?? ""
  );
}

/** Lay out `message` as RFC 5322 (multipart when it has HTML or files). */
function rfc822(message: DemoMessage, date: Date, messageId: string, parent?: string): string {
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
    wrap64(message.text),
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
  let type: string;
  let body: string;
  if (files.length === 0 && message.html === undefined) {
    type = "text/plain; charset=utf-8";
    headers.push(["Content-Transfer-Encoding", "base64"]);
    body = wrap64(message.text);
  } else if (files.length === 0) {
    type = `multipart/alternative; boundary="${alt}"`;
    body = alternative;
  } else {
    const mixed = `mixed-${hex(6)}`;
    type = `multipart/mixed; boundary="${mixed}"`;
    const first =
      message.html === undefined
        ? textPart
        : [`Content-Type: multipart/alternative; boundary="${alt}"`, "", alternative].join(CRLF);
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

/** A new demo mailbox for `address`. */
export function createMailbox(address: string, images: string): Mailbox {
  const mailbox: Mailbox = {
    address,
    messages: [],
    history: [],
    baseHistoryId: 1000,
    historyId: 1000,
  };
  const threads = new Map<string, { threadId: string; messageId: string }>();
  const now = Date.now();
  for (const demo of demoMessages(address, images)) {
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
    const max = Number(url.searchParams.get("maxResults") ?? "100");
    const offset = Number(url.searchParams.get("pageToken") ?? "0");
    const matching = mailbox.messages
      .filter((message) => after === undefined || message.internalDate > Number(after) * 1000)
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

/** The demo pictures the newsletters link to (served under `/images/`). */
export function demoImage(name: string): { type: string; body: string } | undefined {
  const svg = (width: number, height: number, fill: string, label: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="${String(width)}" height="${String(height)}" viewBox="0 0 ${String(width)} ${String(height)}"><rect width="100%" height="100%" rx="8" fill="${fill}"/><text x="50%" y="55%" text-anchor="middle" font-family="Georgia,serif" font-size="${String(Math.round(height / 4))}" fill="#fff">${label}</text></svg>`;
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
