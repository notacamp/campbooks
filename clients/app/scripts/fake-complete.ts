// A fake `complete` model for recordings and local development only: an
// OpenAI-compatible `POST /v1/chat/completions` that answers Scout's reply
// requests (campbooks-cloud#168) and need summaries (campbooks-cloud#204) with
// canned, fictional text, so they can be shown without a real model reading
// anyone's mail. Point the
// server at it:
//
//   pnpm --dir clients fake-complete            # :9060
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete … cargo run -p campbooks-server -- all
//
// It reads only the request's schema name and the JSON the server sends (the
// other person's name, the intent, `shorter`, `change`; a summary's sender,
// subject and sent time), never the mail's words, and keeps nothing. Any other
// request gets 404. A summary's dates (campbooks-cloud#275) are null, except
// for the demo mailbox's sign-in link (`SIGN_IN_SUBJECT`), which lapses ten
// minutes after it was sent, as a real model would read it.
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { parseArgs } from "node:util";

const { values } = parseArgs({ options: { port: { type: "string", default: "9060" } } });
const port = Number(values.port);

interface ThreadMessage {
  from: "them" | "me";
  name: string;
}

interface ReplyInput {
  conversation?: { messages?: ThreadMessage[] };
  intent?: string;
  my_name?: string | null;
  draft?: string;
  change?: string;
  shorter?: boolean;
}

interface SummaryInput {
  from?: string;
  subject?: string;
  sent_at?: string;
}

/** The demo mailbox's sign-in link (`fake-gmail.ts`, `--stale-asks`). */
const SIGN_IN_SUBJECT = "Your Brightloop sign-in link";

/** When the demo sign-in link lapses: ten minutes after it was sent. */
function lapsesAt(input: SummaryInput): string | null {
  if (input.subject !== SIGN_IN_SUBJECT || input.sent_at === undefined) {
    return null;
  }
  return new Date(new Date(input.sent_at).getTime() + 10 * 60_000).toISOString();
}

/** The same canned summary in each shipped language, from the sender and subject only. */
function summary(input: SummaryInput): Record<string, string | null> {
  const from = input.from ?? "Someone";
  const subject = input.subject ?? "";
  if (subject === SIGN_IN_SUBJECT) {
    return {
      en: `${from} sent a sign-in link, valid for 10 minutes after it was sent.`,
      "pt-PT": `${from} enviou uma ligação de início de sessão, válida durante 10 minutos após o envio.`,
      "es-ES": `${from} envió un enlace de inicio de sesión, válido durante 10 minutos tras el envío.`,
      fr: `${from} a envoyé un lien de connexion, valable 10 minutes après son envoi.`,
      lapses_at: lapsesAt(input),
      due_on: null,
    };
  }
  return {
    en: `${from} wrote about “${subject}” and is waiting for your answer.`,
    "pt-PT": `${from} escreveu sobre “${subject}” e está à espera da sua resposta.`,
    "es-ES": `${from} ha escrito sobre «${subject}» y espera tu respuesta.`,
    fr: `${from} a écrit au sujet de « ${subject} » et attend votre réponse.`,
    lapses_at: null,
    due_on: null,
  };
}

interface ChatRequest {
  messages?: { role: string; content: string }[];
  response_format?: { json_schema?: { name?: string } };
}

function firstName(input: ReplyInput): string {
  const them = (input.conversation?.messages ?? []).findLast((m) => m.from === "them");
  return them?.name.split(/\s+/u, 1)[0] ?? "there";
}

function sentence(intent: string): string {
  const trimmed = intent.trim().replace(/^say\s+/iu, "");
  const text = trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
  return /[.!?]$/u.test(text) ? text : `${text}.`;
}

function reply(input: ReplyInput): string {
  const name = firstName(input);
  const signature = input.my_name ?? "";
  const sign = signature === "" ? "" : `\n\n${signature}`;
  if (input.draft !== undefined && input.shorter === true) {
    return `Hi ${name},\n\n${sentence(input.intent ?? "Thanks")}${sign}`;
  }
  if (input.draft !== undefined && input.change !== undefined) {
    return `Hi ${name},\n\nThank you so much for this. ${sentence(input.intent ?? "Thanks")} (${input.change})\n\nBest,${sign}`;
  }
  return `Hi ${name},\n\nThanks for your message. ${sentence(input.intent ?? "Thanks")} Let me know if anything changes.\n\nBest,${sign}`;
}

function answer(request: ChatRequest): unknown {
  const schema = request.response_format?.json_schema?.name;
  const content = request.messages?.find((m) => m.role === "user")?.content ?? "{}";
  const input = JSON.parse(content) as ReplyInput;
  if (schema === "reply_intents") {
    return {
      intents: [
        "Say yes, that works",
        "Ask to move it to the afternoon",
        "Say you'll confirm by tomorrow",
      ],
    };
  }
  if (schema === "reply_draft") {
    return { reply: reply(input) };
  }
  if (schema === "need_summary") {
    return summary((JSON.parse(content) as { email?: SummaryInput }).email ?? {});
  }
  return undefined;
}

function send(response: ServerResponse, status: number, body: unknown): void {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

const server = createServer((request: IncomingMessage, response: ServerResponse) => {
  if (request.method !== "POST" || request.url !== "/v1/chat/completions") {
    send(response, 404, { error: "not found" });
    return;
  }
  const chunks: Buffer[] = [];
  request.on("data", (chunk: Buffer) => chunks.push(chunk));
  request.on("end", () => {
    try {
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as ChatRequest;
      const content = answer(body);
      if (content === undefined) {
        send(response, 404, { error: "only Scout's reply and summary requests are answered" });
        return;
      }
      // A moment, as a model would take.
      setTimeout(() => {
        send(response, 200, {
          model: "fake-complete",
          choices: [
            {
              message: { role: "assistant", content: JSON.stringify(content) },
              finish_reason: "stop",
            },
          ],
          usage: { prompt_tokens: 0, completion_tokens: 0 },
        });
      }, 700);
    } catch {
      send(response, 400, { error: "bad request" });
    }
  });
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`fake complete model on http://127.0.0.1:${String(port)}/v1\n`);
});
