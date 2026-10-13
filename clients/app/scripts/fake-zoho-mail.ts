// The Zoho Mail API of the fake Zoho (scripts/fake-provider.ts --provider
// zoho): a demo mailbox per fake Zoho login, served as the connectors
// module's Zoho adapter reads it (accounts, folders, messages/view, details,
// originalmessage) and writes to it (updatemessage, send and reply,
// attachments). Like Zoho, listings hide archived messages, metadata is
// HTML-escaped, ids are long numbers, and a send sets its own Message-ID.
// Development and recordings only. Every person, company and address here
// is fictional; domains are `.example`.
import { randomInt } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";

import { pdf, rfc822, type DemoMessage } from "./fake-gmail.ts";

/** One stored message. */
interface ZohoMessage {
  id: string;
  folderId: string;
  read: boolean;
  flag: string;
  archived: boolean;
  /** Epoch milliseconds. */
  received: number;
  /** RFC 5322 text. */
  raw: string;
}

export interface ZohoMailbox {
  accountId: string;
  address: string;
  messages: ZohoMessage[];
}

const FOLDERS = [
  { folderId: "9000000002014", folderName: "Inbox", folderType: "Inbox", path: "/Inbox" },
  { folderId: "9000000002016", folderName: "Drafts", folderType: "Drafts", path: "/Drafts" },
  { folderId: "9000000002022", folderName: "Sent", folderType: "Sent", path: "/Sent" },
  { folderId: "9000000002024", folderName: "Spam", folderType: "Spam", path: "/Spam" },
  { folderId: "9000000002026", folderName: "Trash", folderType: "Trash", path: "/Trash" },
  { folderId: "9000000075001", folderName: "Suppliers", folderType: "Inbox", path: "/Suppliers" },
];
const INBOX = "9000000002014";
const SENT = "9000000002022";
const SUPPLIERS = "9000000075001";

/** A long numeric id, as Zoho's. */
const longId = () => `17${String(Date.now())}${String(randomInt(100_000, 999_999))}`;

interface ZohoDemo extends DemoMessage {
  folder: string;
  read: boolean;
  flagged?: boolean;
}

/** The demo mailbox of the Zoho login, newest last. */
function demoMessages(me: string): ZohoDemo[] {
  return [
    {
      from: "Inês Duarte <ines@duarte-contabilidade.example>",
      to: me,
      subject: "IVA do 3.º trimestre: faltam dois recibos",
      hoursAgo: 40,
      thread: "iva",
      folder: INBOX,
      read: true,
      labels: [],
      text: "Olá Sam,\n\nPara fechar o IVA do 3.º trimestre faltam-me os recibos de agosto da Papelaria Sol. Consegue enviar até sexta?\n\nInês",
    },
    {
      from: "Fornecedora Lux <faturas@lux-papel.example>",
      to: me,
      subject: "Fatura FT 2026/318 — 412,50 €",
      hoursAgo: 30,
      folder: SUPPLIERS,
      read: true,
      labels: [],
      text: "Segue em anexo a fatura FT 2026/318 no valor de 412,50 €, com vencimento a 30 de outubro.",
      files: [
        { name: "FT-2026-318.pdf", type: "application/pdf", content: pdf("Fatura FT 2026/318") },
      ],
    },
    {
      from: "Correio do Comércio <newsletter@correio-comercio.example>",
      to: me,
      subject: "Cinco prazos fiscais até ao fim do ano",
      hoursAgo: 22,
      folder: INBOX,
      read: false,
      labels: [],
      headers: {
        "List-Unsubscribe": "<https://correio-comercio.example/sair?u=demo>",
        "List-Id": "Correio do Comercio <semanal.correio-comercio.example>",
        Precedence: "bulk",
      },
      text: "Cinco prazos fiscais até ao fim do ano, e o que muda para trabalhadores independentes em 2027.",
      html: '<h1>Cinco prazos fiscais</h1><p>O que muda para trabalhadores independentes em 2027.</p><p><a href="https://correio-comercio.example/sair?u=demo">Sair da lista</a></p>',
    },
    {
      from: "Hugo Martins <hugo@atelier-martins.example>",
      to: me,
      subject: "Orçamento para a montra de novembro",
      hoursAgo: 8,
      thread: "montra",
      folder: INBOX,
      read: false,
      flagged: true,
      labels: [],
      text: "Bom dia Sam,\n\nSegue o orçamento para a montra de novembro. Se avançarmos esta semana, conseguimos montar a 3.\n\nHugo",
      html: "<p>Bom dia Sam,</p><p>Segue o orçamento para a montra de novembro. Se avançarmos esta semana, conseguimos montar a <b>3</b>.</p><p>Hugo</p>",
      files: [
        { name: "orcamento-montra.pdf", type: "application/pdf", content: pdf("Orcamento montra") },
      ],
    },
    {
      from: "Zoho Mail <notifications@zoho-demo.example>",
      to: me,
      subject: "Nova sessão iniciada no seu correio",
      hoursAgo: 2,
      folder: INBOX,
      read: false,
      labels: [],
      headers: { "Auto-Submitted": "auto-generated" },
      text: "Foi iniciada uma nova sessão no seu correio a partir de Lisboa. Se foi você, não precisa de fazer nada.",
    },
  ];
}

/** A new demo mailbox for `address`. */
export function createZohoMailbox(address: string): ZohoMailbox {
  const mailbox: ZohoMailbox = { accountId: longId(), address, messages: [] };
  const threads = new Map<string, string>();
  const now = Date.now();
  for (const demo of demoMessages(address)) {
    const date = new Date(now - demo.hoursAgo * 3_600_000);
    const messageId = `<${longId()}@zoho-demo.example>`;
    const parent = demo.replyTo === undefined ? undefined : threads.get(demo.replyTo);
    mailbox.messages.push({
      id: longId(),
      folderId: demo.folder,
      read: demo.read,
      flag: demo.flagged === true ? "important" : "flag_not_set",
      archived: false,
      received: date.getTime(),
      raw: rfc822(demo, date, messageId, parent),
    });
    if (demo.thread !== undefined && !threads.has(demo.thread)) {
      threads.set(demo.thread, messageId);
    }
  }
  return mailbox;
}

/** A message that arrives while the app is open (`POST /fake/deliver`). */
export function deliverZohoDemo(mailbox: ZohoMailbox): string {
  const date = new Date();
  const message: ZohoMessage = {
    id: longId(),
    folderId: INBOX,
    read: false,
    flag: "flag_not_set",
    archived: false,
    received: date.getTime(),
    raw: rfc822(
      {
        from: "Inês Duarte <ines@duarte-contabilidade.example>",
        to: mailbox.address,
        subject: "Recibos recebidos, obrigada",
        hoursAgo: 0,
        labels: [],
        text: "Recebi os recibos. O IVA fica entregue amanhã.\n\nInês",
      },
      date,
      `<${longId()}@zoho-demo.example>`,
    ),
  };
  mailbox.messages.push(message);
  return message.id;
}

const escapeHtml = (text: string) =>
  text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");

/** A header of `raw`, as written. */
function header(raw: string, name: string): string {
  const head = raw.split("\r\n\r\n", 1)[0] ?? "";
  const line = head.split("\r\n").find((l) => l.toLowerCase().startsWith(`${name.toLowerCase()}:`));
  return line === undefined ? "" : line.slice(name.length + 1).trim();
}

function listed(message: ZohoMessage) {
  return {
    messageId: message.id,
    folderId: message.folderId,
    receivedTime: String(message.received),
    status: message.read ? "1" : "0",
    flagid: message.flag,
    size: String(message.raw.length),
    subject: escapeHtml(header(message.raw, "Subject")),
    toAddress: escapeHtml(header(message.raw, "To")),
    sender: escapeHtml(header(message.raw, "From")),
  };
}

function reply(response: ServerResponse, status: number, data: unknown) {
  response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  response.end(
    JSON.stringify({
      status: { code: status, description: status < 400 ? "success" : "failure" },
      data,
    }),
  );
}

/** Ids as Zoho takes them: numbers (or numeric strings), alone or listed. */
const ids = (value: unknown): string[] =>
  (Array.isArray(value) ? value : [value])
    .filter((id): id is number | string => typeof id === "number" || typeof id === "string")
    .map(String);

/**
 * Parse a JSON body, keeping long ids (beyond JavaScript's safe integers) as
 * strings: Zoho's ids are 64-bit and the server sends them as numbers.
 */
function parseBody(raw: Buffer): Record<string, unknown> {
  const text = raw.toString("utf8").replaceAll(/([:[,]\s*)(\d{16,})/gu, '$1"$2"');
  return JSON.parse(text || "{}") as Record<string, unknown>;
}

async function bodyOf(request: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk as Uint8Array));
  return Buffer.concat(chunks);
}

/** What the demo mailbox holds, newest first: each message's subject and state. */
export function describeZohoMailbox(mailbox: ZohoMailbox) {
  return mailbox.messages
    .toSorted((a, b) => b.received - a.received)
    .map((m) => ({
      subject: header(m.raw, "Subject"),
      folder: FOLDERS.find((f) => f.folderId === m.folderId)?.folderName ?? m.folderId,
      read: m.read,
      flag: m.flag,
      archived: m.archived,
    }));
}

/**
 * Answer a Zoho Mail API request (`/api/…`) for the login whose mailbox is
 * `mailbox`. Returns false when the path is not one this fake serves.
 */
export async function handleZohoMail(
  request: IncomingMessage,
  url: URL,
  response: ServerResponse,
  mailbox: ZohoMailbox,
): Promise<boolean> {
  const path = url.pathname.replace(/^\/api\//u, "");
  if (path === "accounts" && request.method === "GET") {
    reply(response, 200, [
      {
        accountId: mailbox.accountId,
        primaryEmailAddress: mailbox.address,
        displayName: "Sam Rivera",
        type: "ZOHO_ACCOUNT",
      },
    ]);
    return true;
  }
  const match = /^accounts\/(\d+)\/(.*)$/u.exec(path);
  if (match?.[1] !== mailbox.accountId) {
    return false;
  }
  const rest = match[2] ?? "";
  const find = (id: string) => mailbox.messages.find((m) => m.id === id);
  if (rest === "folders" && request.method === "GET") {
    reply(response, 200, FOLDERS);
    return true;
  }
  if (rest === "messages/view" && request.method === "GET") {
    const folder = url.searchParams.get("folderId") ?? "";
    const start = Number(url.searchParams.get("start") ?? "1");
    const limit = Number(url.searchParams.get("limit") ?? "10");
    const page = mailbox.messages
      .filter((m) => m.folderId === folder && !m.archived)
      .toSorted((a, b) => b.received - a.received)
      .slice(start - 1, start - 1 + limit)
      .map(listed);
    reply(response, 200, page);
    return true;
  }
  const details = /^folders\/\d+\/messages\/(\d+)\/details$/u.exec(rest)?.[1];
  if (details !== undefined) {
    const message = find(details);
    reply(response, message ? 200 : 404, message ? listed(message) : { errorCode: "NOT_FOUND" });
    return true;
  }
  const original = /^messages\/(\d+)\/originalmessage$/u.exec(rest)?.[1];
  if (original !== undefined) {
    const message = find(original);
    reply(
      response,
      message ? 200 : 404,
      message ? { messageId: message.id, content: message.raw } : { errorCode: "NOT_FOUND" },
    );
    return true;
  }
  if (rest === "updatemessage" && request.method === "PUT") {
    const body = parseBody(await bodyOf(request));
    const targets = ids(body["messageId"]).map(find);
    if (targets.includes(undefined)) {
      reply(response, 404, { errorCode: "MESSAGE_NOT_FOUND" });
      return true;
    }
    for (const message of targets) {
      if (message === undefined) continue;
      switch (body["mode"]) {
        case "markAsRead": {
          message.read = true;
          break;
        }
        case "markAsUnread": {
          message.read = false;
          break;
        }
        case "setFlag": {
          message.flag = String(body["flagid"]);
          break;
        }
        case "archiveMails": {
          message.archived = true;
          break;
        }
        case "unArchiveMails": {
          message.archived = false;
          break;
        }
        case "moveMessage": {
          message.folderId = String(body["destfolderId"]);
          break;
        }
        default: {
          reply(response, 400, { errorCode: "INVALID_MODE" });
          return true;
        }
      }
      console.log(`fake Zoho Mail: ${message.id} ${body["mode"] as string}`);
    }
    reply(response, 200, null);
    return true;
  }
  if (rest === "messages/attachments" && request.method === "POST") {
    const size = (await bodyOf(request)).length;
    const name = url.searchParams.get("fileName") ?? "attachment";
    reply(response, 200, {
      storeName: "52882865",
      attachmentPath: `/compose/${String(size)}-${name}`,
      attachmentName: name,
    });
    return true;
  }
  const sendTo = /^messages(?:\/(\d+))?$/u.exec(rest);
  if (sendTo !== null && request.method === "POST") {
    const body = parseBody(await bodyOf(request));
    const parent = sendTo[1] === undefined ? undefined : find(sendTo[1]);
    if (sendTo[1] !== undefined && (parent === undefined || body["action"] !== "reply")) {
      reply(response, 400, { errorCode: "INVALID_REPLY" });
      return true;
    }
    const field = (key: string) => (typeof body[key] === "string" ? body[key] : "");
    const date = new Date();
    const parentId = parent === undefined ? undefined : header(parent.raw, "Message-ID");
    const sent: ZohoMessage = {
      id: longId(),
      folderId: SENT,
      read: true,
      flag: "flag_not_set",
      archived: false,
      received: date.getTime(),
      raw: rfc822(
        {
          from: `Sam Rivera <${field("fromAddress")}>`,
          to: field("toAddress"),
          subject: field("subject"),
          hoursAgo: 0,
          labels: [],
          text: field("mailFormat") === "plaintext" ? field("content") : "",
          ...(field("mailFormat") === "plaintext" ? {} : { html: field("content") }),
        },
        date,
        `<${longId()}@zoho-demo.example>`,
        parentId,
      ),
    };
    mailbox.messages.push(sent);
    console.log(
      `fake Zoho Mail: sent ${sent.id}${parent ? ` in reply to ${parent.id}` : ""} (not delivered)`,
    );
    reply(response, 200, { messageId: sent.id, subject: field("subject") });
    return true;
  }
  return false;
}
