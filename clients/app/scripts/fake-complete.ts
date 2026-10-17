// A fake `complete` model for recordings and local development only: an
// OpenAI-compatible `POST /v1/chat/completions` that answers Scout's reply
// requests (campbooks-cloud#168) with canned, fictional text, so the reply
// dialog can be shown without a real model reading anyone's mail. Point the
// server at it:
//
//   pnpm --dir clients fake-complete            # :9060
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete … cargo run -p campbooks-server -- all
//
// It reads only the request's schema name and the JSON the server sends (the
// other person's name, the intent, `shorter`, `change`), never anything else,
// and keeps nothing. Any other request gets 404.
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
        send(response, 404, { error: "only Scout's reply requests are answered" });
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
