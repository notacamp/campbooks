// A fake Google for recordings and local development only: an OAuth consent
// page, a token endpoint (authorization code + PKCE S256) and a JWKS, laid
// out like Google under one base URL. The server talks to it only with
// CAMPBOOKS_ENV=development and CAMPBOOKS_GOOGLE__BASE_URL set to its address
// (the server refuses that setting otherwise).
//
//   pnpm --dir clients fake-google [--port 9050]
//
// Everyone who signs in is the same fictional person, Sam Rivera; each new
// browser (a cookie on this origin) is a different Google account of hers,
// so every recording run starts as someone Campbooks has not met.
import { createHash, generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { parseArgs } from "node:util";

const { values } = parseArgs({ options: { port: { type: "string", default: "9050" } } });
const port = Number(values.port);
const issuer = `http://127.0.0.1:${String(port)}`;
const person = { email: "sam.rivera@example.com", name: "Sam Rivera" };

const b64url = (data: Buffer | string) => Buffer.from(data).toString("base64url");

// A fresh key (and key id) per run, so a server that cached an earlier run's
// keys fetches these.
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const kid = `fake-google-${b64url(randomBytes(6))}`;
const jwk = { ...publicKey.export({ format: "jwk" }), kid, use: "sig", alg: "EdDSA" };

interface Grant {
  sub: string;
  clientId: string;
  redirectUri: string;
  scope: string;
  nonce: string;
  challenge: string;
}
const codes = new Map<string, Grant>();

function idToken(sub: string, clientId: string, nonce: string): string {
  const now = Math.floor(Date.now() / 1000);
  const header = b64url(JSON.stringify({ alg: "EdDSA", typ: "JWT", kid }));
  const claims = b64url(
    JSON.stringify({
      iss: issuer,
      aud: clientId,
      azp: clientId,
      sub,
      email: person.email,
      email_verified: true,
      name: person.name,
      nonce,
      iat: now,
      exp: now + 600,
    }),
  );
  const signature = sign(null, Buffer.from(`${header}.${claims}`), privateKey);
  return `${header}.${claims}.${b64url(signature)}`;
}

const escape = (text: string) =>
  text.replaceAll(/[&<>"']/gu, (c) => `&#${String(c.codePointAt(0))};`);

function consentPage(query: URLSearchParams): string {
  const scopes = (query.get("scope") ?? "").split(" ").filter(Boolean);
  const wording: Record<string, string> = {
    openid: "Know who you are on Google",
    email: "See your email address",
    profile: "See your name",
    "https://www.googleapis.com/auth/gmail.modify": "Read, compose and organise your mail",
  };
  const items = scopes.map((s) => `<li>${escape(wording[s] ?? s)}</li>`).join("");
  const allow = new URLSearchParams(query);
  allow.set("decision", "allow");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Fake Google: sign in</title>
<style>
body{margin:0;font:16px/1.5 system-ui,sans-serif;background:#f1f3f4;color:#202124}
main{max-width:26rem;margin:2rem auto;background:#fff;border-radius:12px;padding:1.5rem;box-shadow:0 1px 3px #0003}
.tag{font-size:.75rem;text-transform:uppercase;letter-spacing:.08em;color:#b3261e}
h1{font-size:1.4rem;margin:.25rem 0 1rem}
.who{display:flex;gap:.75rem;align-items:center;padding:.75rem;border:1px solid #dadce0;border-radius:8px}
.avatar{width:2.5rem;height:2.5rem;border-radius:50%;background:#1a73e8;color:#fff;display:grid;place-items:center;font-weight:600}
ul{padding-left:1.2rem} a.allow{display:block;text-align:center;margin-top:1rem;background:#1a73e8;color:#fff;
text-decoration:none;padding:.7rem;border-radius:6px;font-weight:600}
</style></head><body><main>
<div class="tag">Fake Google · development only</div>
<h1>Sign in to Campbooks</h1>
<div class="who"><div class="avatar">SR</div><div><strong>${escape(person.name)}</strong><br>${escape(person.email)}</div></div>
<p>Campbooks wants to:</p><ul>${items}</ul>
<a class="allow" href="/o/oauth2/v2/auth?${escape(allow.toString())}">Allow</a>
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

const server = createServer((request, response) => {
  void (async () => {
    const url = new URL(request.url ?? "/", issuer);
    if (url.pathname === "/o/oauth2/v2/auth") {
      const q = url.searchParams;
      const cookie = /(?:^|;\s*)fg_account=([\w-]+)/u.exec(request.headers.cookie ?? "")?.[1];
      if (q.get("decision") !== "allow") {
        response.writeHead(200, {
          "content-type": "text/html; charset=utf-8",
          "set-cookie": `fg_account=${cookie ?? `demo-${b64url(randomBytes(6))}`}; Path=/; HttpOnly; Secure; SameSite=Lax`,
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
      });
      const back = new URL(q.get("redirect_uri") ?? "");
      back.searchParams.set("code", code);
      back.searchParams.set("state", q.get("state") ?? "");
      response.writeHead(302, { location: back.toString() });
      response.end();
      return;
    }
    if (url.pathname === "/token" && request.method === "POST") {
      const form = await body(request);
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
      json(response, 200, {
        access_token: `fake-access-${b64url(randomBytes(12))}`,
        refresh_token: `fake-refresh-${b64url(randomBytes(12))}`,
        expires_in: 3599,
        token_type: "Bearer",
        scope: grant.scope,
        id_token: idToken(grant.sub, grant.clientId, grant.nonce),
      });
      return;
    }
    if (url.pathname === "/oauth2/v3/certs") {
      json(response, 200, { keys: [jwk] });
      return;
    }
    json(response, 404, { error: "not_found" });
  })();
});

server.listen(port, "127.0.0.1", () => {
  console.log(`fake Google (development only) on ${issuer}`);
});
