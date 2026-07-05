#!/usr/bin/env python3
"""
Campbooks local OAuth helper.

Runs a loopback OAuth flow and prints the obtained refresh token to stdout as JSON.
Human-readable messages go to stderr; only the final JSON goes to stdout, so you can
pipe or capture it cleanly.

IMPORTANT: You must use the SERVER's own OAuth client credentials — the same
client_id/secret that your Campbooks instance is configured with. A refresh token
minted under a different client_id will fail when the server tries to use it.

Usage:
  python3 campbooks_oauth.py google [--client-id ID] [--client-secret SECRET] [--port PORT]
  python3 campbooks_oauth.py zoho   [--client-id ID] [--client-secret SECRET] [--port PORT]
                                    [--region REGION]

Zoho regions: eu (default), us, in, au, jp, ca, cn, sa

The printed JSON looks like:
  {"provider": "google", "refresh_token": "1//0e..."}

Pass refresh_token to connect_email_account(mode="token", ...) via the MCP API.
"""

import argparse
import getpass
import http.server
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import webbrowser

# ---------------------------------------------------------------------------
# Google
# ---------------------------------------------------------------------------

GOOGLE_AUTH_URL = "https://accounts.google.com/o/oauth2/v2/auth"
GOOGLE_TOKEN_URL = "https://oauth2.googleapis.com/token"
# Scopes mirror Google::OauthClient::CONNECT_SCOPES used by the Campbooks server.
GOOGLE_SCOPES = " ".join([
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/gmail.modify",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/calendar.events",
])

# ---------------------------------------------------------------------------
# Zoho
# ---------------------------------------------------------------------------

ZOHO_REGION_DOMAINS = {
    "eu":  "zoho.eu",
    "us":  "zoho.com",
    "com": "zoho.com",
    "in":  "zoho.in",
    "au":  "zoho.com.au",
    "jp":  "zoho.jp",
    "ca":  "zohocloud.ca",
    "cn":  "zoho.com.cn",
    "sa":  "zoho.sa",
}
# Scopes mirror Zoho::OauthClient scope string used by the Campbooks server.
ZOHO_SCOPE = (
    "ZohoMail.messages.ALL,ZohoMail.attachments.READ,"
    "ZohoMail.accounts.READ,ZohoMail.folders.READ,"
    "ZohoMail.tags.ALL,ZohoCalendar.event.ALL,ZohoCalendar.calendar.ALL"
)

CALLBACK_TIMEOUT = 120  # seconds


def eprint(*args, **kwargs):
    """Write to stderr."""
    print(*args, file=sys.stderr, **kwargs)


# ---------------------------------------------------------------------------
# Local callback server
# ---------------------------------------------------------------------------

def _run_callback_server(port):
    """
    Start a one-shot HTTP server on localhost:port, wait for a single /callback
    request, and return the authorization code.  Calls sys.exit on error or timeout.
    """
    result = {"code": None, "error": None, "error_description": None}

    class _Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            if not self.path.startswith("/callback"):
                self._respond("Unexpected request — waiting for /callback.")
                return
            params = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
            if "code" in params:
                result["code"] = params["code"][0]
                self._respond("Authorization successful. You can close this tab.")
            elif "error" in params:
                result["error"] = params["error"][0]
                result["error_description"] = params.get(
                    "error_description", [params["error"][0]]
                )[0]
                self._respond(
                    f"Authorization error: {params['error'][0]}. You can close this tab."
                )
            else:
                self._respond("Unexpected callback parameters. You can close this tab.")

        def _respond(self, message):
            body = message.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass  # suppress access log

    httpd = http.server.HTTPServer(("localhost", port), _Handler)
    deadline = time.monotonic() + CALLBACK_TIMEOUT

    while result["code"] is None and result["error"] is None:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            httpd.server_close()
            eprint(f"\nTimed out after {CALLBACK_TIMEOUT} s waiting for the browser callback.")
            eprint("Make sure you completed the authorization in the browser window.")
            sys.exit(1)
        httpd.timeout = min(remaining, 5.0)
        httpd.handle_request()

    httpd.server_close()

    if result["error"]:
        eprint(f"\nAuthorization error: {result['error_description']}")
        sys.exit(1)

    return result["code"]


# ---------------------------------------------------------------------------
# Token exchange
# ---------------------------------------------------------------------------

def _exchange_code(token_url, client_id, client_secret, code, redirect_uri):
    """POST the authorization code and return the full token response dict."""
    payload = urllib.parse.urlencode({
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": redirect_uri,
        "client_id": client_id,
        "client_secret": client_secret,
    }).encode("ascii")
    req = urllib.request.Request(token_url, data=payload, method="POST")
    req.add_header("Content-Type", "application/x-www-form-urlencoded")
    req.add_header("Accept", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        eprint(f"\nToken exchange failed (HTTP {exc.code}):\n{body}")
        if "redirect_uri_mismatch" in body or "redirect_uri" in body.lower():
            eprint(
                f"\nHint: The server's OAuth client does not list '{redirect_uri}'"
                " as an allowed redirect URI.\n"
                f"      Add it in your OAuth app settings, then retry."
            )
        sys.exit(1)
    except Exception as exc:  # noqa: BLE001
        eprint(f"\nToken exchange failed: {exc}")
        sys.exit(1)


# ---------------------------------------------------------------------------
# Main flow
# ---------------------------------------------------------------------------

def run_flow(provider, client_id, client_secret, port, region="eu"):
    redirect_uri = f"http://localhost:{port}/callback"

    if provider == "google":
        auth_url = GOOGLE_AUTH_URL
        token_url = GOOGLE_TOKEN_URL
        params = {
            "client_id":     client_id,
            "redirect_uri":  redirect_uri,
            "response_type": "code",
            "scope":         GOOGLE_SCOPES,
            "access_type":   "offline",
            "prompt":        "consent",
        }
    elif provider == "zoho":
        domain = ZOHO_REGION_DOMAINS.get(region.lower())
        if not domain:
            valid = ", ".join(sorted(ZOHO_REGION_DOMAINS))
            eprint(f"Unknown Zoho region '{region}'. Valid values: {valid}")
            sys.exit(1)
        auth_url  = f"https://accounts.{domain}/oauth/v2/auth"
        token_url = f"https://accounts.{domain}/oauth/v2/token"
        params = {
            "client_id":     client_id,
            "redirect_uri":  redirect_uri,
            "response_type": "code",
            "scope":         ZOHO_SCOPE,
            "access_type":   "offline",
            "prompt":        "consent",
        }
    else:
        eprint(f"Unknown provider '{provider}'. Use 'google' or 'zoho'.")
        sys.exit(1)

    full_auth_url = auth_url + "?" + urllib.parse.urlencode(params, quote_via=urllib.parse.quote)

    eprint(f"\nOpening your browser for {provider} authorization...")
    eprint(f"If the browser does not open automatically, navigate to:\n  {full_auth_url}\n")
    webbrowser.open(full_auth_url)

    code = _run_callback_server(port)

    eprint("Exchanging authorization code for refresh token...")
    token_data = _exchange_code(token_url, client_id, client_secret, code, redirect_uri)

    refresh_token = token_data.get("refresh_token")
    if not refresh_token:
        eprint("\nNo refresh_token in the token response.")
        eprint(
            "This can happen when the OAuth app was already authorized without"
            " access_type=offline + prompt=consent.\n"
            "Both are set automatically — try revoking the app's access in your"
            " account settings, then run this script again."
        )
        sys.exit(1)

    return refresh_token


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "provider",
        choices=["google", "zoho"],
        help="OAuth provider to authorize",
    )
    parser.add_argument(
        "--client-id",
        metavar="ID",
        help="OAuth client ID (prompted securely if not given)",
    )
    parser.add_argument(
        "--client-secret",
        metavar="SECRET",
        help="OAuth client secret (prompted securely if not given)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8765,
        help="Local port for the callback server (default: 8765)",
    )
    parser.add_argument(
        "--region",
        default="eu",
        metavar="REGION",
        help="Zoho data-centre region: eu (default), us, in, au, jp, ca, cn, sa",
    )
    args = parser.parse_args()

    client_id = args.client_id
    if not client_id:
        client_id = getpass.getpass("Client ID: ")
    client_secret = args.client_secret
    if not client_secret:
        client_secret = getpass.getpass("Client secret: ")

    if not client_id:
        eprint("Error: client ID is required.")
        sys.exit(1)
    if not client_secret:
        eprint("Error: client secret is required.")
        sys.exit(1)

    refresh_token = run_flow(args.provider, client_id, client_secret, args.port, args.region)

    # Only JSON goes to stdout.
    print(json.dumps({"provider": args.provider, "refresh_token": refresh_token}))

    eprint(
        "\nDone. Pass the refresh_token value to connect_email_account(mode='token', ...)."
    )
    eprint("Keep it secure — it grants full mailbox access to anyone who has it.")


if __name__ == "__main__":
    main()
