// A fake Google, Microsoft or Zoho for recordings and local development only:
// a consent page, a token endpoint (authorization code + PKCE S256) and a key
// set, laid out like the real provider under one base URL. The server talks to
// it only with CAMPBOOKS_ENV=development and CAMPBOOKS_<PROVIDER>__BASE_URL set
// to its address (the server refuses that setting otherwise).
//
//   pnpm --dir clients fake-google      # :9050
//   pnpm --dir clients fake-microsoft   # :9051
//   pnpm --dir clients fake-zoho        # :9052
//
// Everyone who signs in is the same fictional person, Sam Rivera; each new
// browser (a cookie on this origin) is a different account of hers at that
// provider, so every recording run starts as someone Campbooks has not met.
//
// - Google: issuer = the base URL; `email_verified` true.
// - Microsoft: issuer = `<base>/<tid>/v2.0` for one fictional tenant, as the
//   real `common` endpoint does; no `email_verified` (Microsoft never sends it).
// - Zoho: one data centre (the base URL); the callback names it in
//   `accounts-server`, as Zoho's does.
//
// Each fake Google account also has a demo Gmail mailbox (scripts/
// fake-gmail.ts), and each fake Zoho login a demo Zoho Mail mailbox (scripts/
// fake-zoho-mail.ts), that the connectors module syncs, writes members'
// changes back to and sends through: `POST /fake/deliver` drops one more
// message into every mailbox, to show mail arriving live, and
// `GET /fake/mailbox` lists what each holds (subjects and state), to show
// write-backs landing.
import { createHash, generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { parseArgs } from "node:util";

import {
  createMailbox,
  deliverDemo,
  demoImage,
  describeMailbox,
  handleGmail,
  sendMessage,
  type Mailbox,
  writeGmail,
} from "./fake-gmail.ts";
import {
  createZohoMailbox,
  deliverZohoDemo,
  describeZohoMailbox,
  handleZohoMail,
  type ZohoMailbox,
} from "./fake-zoho-mail.ts";

type Provider = "google" | "microsoft" | "zoho";

const { values } = parseArgs({
  options: {
    provider: { type: "string", default: "google" },
    port: { type: "string" },
    // Extra issues of a demo newsletter, so teaching once has lookalikes.
    lookalikes: { type: "string", default: "0" },
  },
});
const provider = values.provider as Provider;
const DEFAULT_PORTS: Record<Provider, string> = { google: "9050", microsoft: "9051", zoho: "9052" };
if (!(provider in DEFAULT_PORTS)) {
  throw new Error(`--provider must be google, microsoft or zoho, not ${values.provider}`);
}
const port = Number(values.port ?? DEFAULT_PORTS[provider]);
const base = `http://127.0.0.1:${String(port)}`;
/** The fictional Microsoft tenant everyone belongs to. */
const TENANT = "2b5a2c1e-6f1d-4c8e-9a3b-7d4e5f6a7b8c";
const NAMES: Record<Provider, string> = { google: "Google", microsoft: "Microsoft", zoho: "Zoho" };

const person = {
  name: "Sam Rivera",
  email: {
    google: "sam.rivera@example.com",
    microsoft: "sam.rivera@example.org",
    zoho: "sam.rivera@example.net",
  }[provider],
};

/** Where each endpoint lives, as at the real provider. */
const layout = {
  google: {
    authorize: "/o/oauth2/v2/auth",
    token: "/token",
    keys: "/oauth2/v3/certs",
    issuer: base,
  },
  microsoft: {
    authorize: "/common/oauth2/v2.0/authorize",
    token: "/common/oauth2/v2.0/token",
    keys: "/common/discovery/v2.0/keys",
    issuer: `${base}/${TENANT}/v2.0`,
  },
  zoho: {
    authorize: "/oauth/v2/auth",
    token: "/oauth/v2/token",
    keys: "/oauth/v2/keys",
    issuer: base,
  },
}[provider];

const b64url = (data: Buffer | string) => Buffer.from(data).toString("base64url");

// A fresh key (and key id) per run, so a server that cached an earlier run's
// keys fetches these.
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const kid = `fake-${provider}-${b64url(randomBytes(6))}`;
const jwk = { ...publicKey.export({ format: "jwk" }), kid, use: "sig", alg: "EdDSA" };

interface Grant {
  sub: string;
  clientId: string;
  redirectUri: string;
  scope: string;
  nonce: string;
  challenge: string;
  /** Whether offline access was asked (a refresh token comes back). */
  offline: boolean;
}
const codes = new Map<string, Grant>();
/** Access and refresh tokens → the account they were issued for. */
const tokens = new Map<string, string>();
const mailboxes = new Map<string, Mailbox>();
const zohoMailboxes = new Map<string, ZohoMailbox>();

function zohoMailboxOf(sub: string): ZohoMailbox {
  let mailbox = zohoMailboxes.get(sub);
  if (mailbox === undefined) {
    mailbox = createZohoMailbox(person.email);
    zohoMailboxes.set(sub, mailbox);
  }
  return mailbox;
}

function mailboxOf(sub: string): Mailbox {
  let mailbox = mailboxes.get(sub);
  if (mailbox === undefined) {
    mailbox = createMailbox(person.email, `${base}/images`, Number(values.lookalikes));
    mailboxes.set(sub, mailbox);
  }
  return mailbox;
}

/** The account a request's bearer token was issued for (Zoho: `Zoho-oauthtoken`). */
function bearer(request: IncomingMessage): string | undefined {
  const token = /^(?:Bearer|Zoho-oauthtoken) (\S+)$/u.exec(
    request.headers.authorization ?? "",
  )?.[1];
  return token === undefined ? undefined : tokens.get(token);
}

/** A new access token (and refresh token, unless one is being reused) for `sub`. */
function issueTokens(sub: string, refresh?: string) {
  const access = `fake-access-${b64url(randomBytes(12))}`;
  tokens.set(access, sub);
  const refreshToken = refresh ?? `fake-refresh-${b64url(randomBytes(12))}`;
  tokens.set(refreshToken, sub);
  return { access, refresh: refreshToken };
}

function idToken(sub: string, clientId: string, nonce: string): string {
  const now = Math.floor(Date.now() / 1000);
  const header = b64url(JSON.stringify({ alg: "EdDSA", typ: "JWT", kid }));
  const claims: Record<string, unknown> = {
    iss: layout.issuer,
    aud: clientId,
    azp: clientId,
    sub,
    email: person.email,
    name: person.name,
    nonce,
    iat: now,
    exp: now + 600,
  };
  if (provider === "microsoft") {
    claims["tid"] = TENANT;
    claims["preferred_username"] = person.email;
  } else {
    claims["email_verified"] = true;
  }
  const body = b64url(JSON.stringify(claims));
  const signature = sign(null, Buffer.from(`${header}.${body}`), privateKey);
  return `${header}.${body}.${b64url(signature)}`;
}

const escape = (text: string) =>
  text.replaceAll(/[&<>"']/gu, (c) => `&#${String(c.codePointAt(0))};`);

function consentPage(query: URLSearchParams): string {
  const scopes = (query.get("scope") ?? "").split(" ").filter(Boolean);
  const wording: Record<string, string> = {
    openid: `Know who you are on ${NAMES[provider]}`,
    email: "See your email address",
    profile: "See your name",
    offline_access: "Keep access when you are away",
    "https://www.googleapis.com/auth/gmail.modify": "Read, compose and organise your mail",
    "ZohoMail.accounts.READ": "See your mail accounts",
    "ZohoMail.folders.READ": "See your folders",
    "ZohoMail.messages.ALL": "Read, send and organise your mail",
  };
  const items = scopes.map((s) => `<li>${escape(wording[s] ?? s)}</li>`).join("");
  const allow = new URLSearchParams(query);
  allow.set("decision", "allow");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Fake ${NAMES[provider]}: sign in</title>
<style>
body{margin:0;font:16px/1.5 system-ui,sans-serif;background:#f1f3f4;color:#202124}
main{max-width:26rem;margin:2rem auto;background:#fff;border-radius:12px;padding:1.5rem;box-shadow:0 1px 3px #0003}
.tag{font-size:.75rem;text-transform:uppercase;letter-spacing:.08em;color:#b3261e}
h1{font-size:1.4rem;margin:.25rem 0 1rem}
.who{display:flex;gap:.75rem;align-items:center;padding:.75rem;border:1px solid #dadce0;border-radius:8px}
.avatar{width:2.5rem;height:2.5rem;border-radius:50%;background:#3c4043;color:#fff;display:grid;place-items:center;font-weight:600}
ul{padding-left:1.2rem} a.allow{display:block;text-align:center;margin-top:1rem;background:#3c4043;color:#fff;
text-decoration:none;padding:.7rem;border-radius:6px;font-weight:600}
</style></head><body><main>
<div class="tag">Fake ${NAMES[provider]} · development only</div>
<h1>Sign in to Campbooks</h1>
<div class="who"><div class="avatar">SR</div><div><strong>${escape(person.name)}</strong><br>${escape(person.email)}</div></div>
<p>Campbooks wants to:</p><ul>${items}</ul>
<a class="allow" href="${layout.authorize}?${escape(allow.toString())}">Allow</a>
</main></body></html>`;
}

async function body(request: IncomingMessage): Promise<URLSearchParams> {
  let data = "";
  for await (const chunk of request) data += String(chunk);
  return new URLSearchParams(data);
}

function json(response: ServerResponse, status: number, value: unknown) {
  response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  response.end(JSON.stringify(value));
}

/** One account per browser, per provider (cookies are not kept apart by port). */
const COOKIE = `fake_${provider}_account`;

const server = createServer((request, response) => {
  void (async () => {
    const url = new URL(request.url ?? "/", base);
    if (url.pathname === layout.authorize) {
      const q = url.searchParams;
      const cookie = new RegExp(String.raw`(?:^|;\s*)${COOKIE}=([\w-]+)`, "u").exec(
        request.headers.cookie ?? "",
      )?.[1];
      if (q.get("decision") !== "allow") {
        response.writeHead(200, {
          "content-type": "text/html; charset=utf-8",
          "set-cookie": `${COOKIE}=${cookie ?? `demo-${b64url(randomBytes(6))}`}; Path=/; HttpOnly; Secure; SameSite=Lax`,
        });
        response.end(consentPage(q));
        return;
      }
      const code = b64url(randomBytes(32));
      codes.set(code, {
        sub: cookie ?? "demo-anonymous",
        clientId: q.get("client_id") ?? "",
        redirectUri: q.get("redirect_uri") ?? "",
        scope: q.get("scope") ?? "",
        nonce: q.get("nonce") ?? "",
        challenge: q.get("code_challenge") ?? "",
        offline:
          provider === "microsoft"
            ? (q.get("scope") ?? "").split(" ").includes("offline_access")
            : q.get("access_type") === "offline",
      });
      const back = new URL(q.get("redirect_uri") ?? "");
      back.searchParams.set("code", code);
      back.searchParams.set("state", q.get("state") ?? "");
      if (provider === "zoho") {
        back.searchParams.set("location", "eu");
        back.searchParams.set("accounts-server", base);
      }
      response.writeHead(302, { location: back.toString() });
      response.end();
      return;
    }
    if (url.pathname === layout.token && request.method === "POST") {
      const form = await body(request);
      if (form.get("grant_type") === "refresh_token") {
        const refresh = form.get("refresh_token") ?? "";
        // A refresh token from an earlier run of the fake still works: it
        // gets a fresh demo mailbox of its own.
        const sub = tokens.get(refresh) ?? (refresh.startsWith("fake-refresh-") ? refresh : "");
        if (sub === "") {
          json(response, 400, { error: "invalid_grant" });
          return;
        }
        const issued = issueTokens(sub, refresh);
        json(response, 200, {
          access_token: issued.access,
          expires_in: 3599,
          token_type: "Bearer",
        });
        return;
      }
      const grant = codes.get(form.get("code") ?? "");
      codes.delete(form.get("code") ?? "");
      const challenge = b64url(
        createHash("sha256")
          .update(form.get("code_verifier") ?? "")
          .digest(),
      );
      const valid =
        grant?.challenge === challenge &&
        grant.redirectUri === form.get("redirect_uri") &&
        grant.clientId === form.get("client_id");
      if (!grant || !valid) {
        json(response, 400, { error: "invalid_grant" });
        return;
      }
      const issued = issueTokens(grant.sub);
      json(response, 200, {
        access_token: issued.access,
        ...(grant.offline ? { refresh_token: issued.refresh } : {}),
        expires_in: 3599,
        token_type: "Bearer",
        // Zoho documents its scopes comma-separated.
        scope: provider === "zoho" ? grant.scope.replaceAll(" ", ",") : grant.scope,
        id_token: idToken(grant.sub, grant.clientId, grant.nonce),
      });
      return;
    }
    if (url.pathname === layout.keys) {
      json(response, 200, { keys: [jwk] });
      return;
    }
    if (provider === "google") {
      if (url.pathname === "/revoke" && request.method === "POST") {
        json(response, 200, {});
        return;
      }
      if (url.pathname === "/v1/userinfo") {
        const sub = bearer(request);
        if (sub === undefined) {
          json(response, 401, { error: "invalid_token" });
          return;
        }
        json(response, 200, { sub, email: person.email, email_verified: true, name: person.name });
        return;
      }
      if (url.pathname.startsWith("/gmail/v1/users/me/")) {
        const sub = bearer(request);
        if (sub === undefined) {
          json(response, 401, { error: { code: 401, message: "Invalid Credentials" } });
          return;
        }
        if (handleGmail(request, url, response, mailboxOf(sub))) {
          return;
        }
        if (await writeGmail(request, url, response, mailboxOf(sub))) {
          return;
        }
      }
      if (url.pathname === "/upload/gmail/v1/users/me/messages/send" && request.method === "POST") {
        const sub = bearer(request);
        if (sub === undefined) {
          json(response, 401, { error: { code: 401, message: "Invalid Credentials" } });
          return;
        }
        const chunks: Buffer[] = [];
        for await (const chunk of request) chunks.push(Buffer.from(chunk as Uint8Array));
        const raw = Buffer.concat(chunks).toString("utf8");
        const answer = sendMessage(mailboxOf(sub), request.headers["content-type"] ?? "", raw);
        json(response, answer.status, answer.value);
        return;
      }
      if (url.pathname.startsWith("/images/")) {
        const image = demoImage(url.pathname.slice("/images/".length));
        if (image !== undefined) {
          response.writeHead(200, { "content-type": image.type, "cache-control": "no-store" });
          response.end(Buffer.from(image.body, "latin1"));
          return;
        }
      }
      if (url.pathname === "/fake/mailbox" && request.method === "GET") {
        // What the demo mailboxes hold (subjects and labels), for recordings.
        json(response, 200, { mailboxes: [...mailboxes.values()].map(describeMailbox) });
        return;
      }
      if (url.pathname === "/fake/deliver" && request.method === "POST") {
        const delivered = [...mailboxes.values()].map((mailbox) => deliverDemo(mailbox).id);
        json(response, 200, { delivered });
        return;
      }
    }
    if (provider === "zoho") {
      if (url.pathname === "/oauth/v2/token/revoke" && request.method === "POST") {
        json(response, 200, {});
        return;
      }
      if (url.pathname.startsWith("/api/")) {
        const sub = bearer(request);
        if (sub === undefined) {
          json(response, 401, { status: { code: 401 }, data: { errorCode: "INVALID_OAUTHTOKEN" } });
          return;
        }
        if (await handleZohoMail(request, url, response, zohoMailboxOf(sub))) {
          return;
        }
      }
      if (url.pathname === "/fake/mailbox" && request.method === "GET") {
        json(response, 200, { mailboxes: [...zohoMailboxes.values()].map(describeZohoMailbox) });
        return;
      }
      if (url.pathname === "/fake/deliver" && request.method === "POST") {
        const delivered = [...zohoMailboxes.values()].map(deliverZohoDemo);
        json(response, 200, { delivered });
        return;
      }
    }
    json(response, 404, { error: "not_found" });
  })();
});

server.listen(port, "127.0.0.1", () => {
  console.log(`fake ${NAMES[provider]} (development only) on ${base}`);
});
