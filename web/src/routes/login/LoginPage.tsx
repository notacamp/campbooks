/**
 * LoginPage — sign-in form at /login.
 *
 * POST /api/app/session { email_address, password }
 *   200 → setToken, invalidate ["me"], navigate to /today.
 *   401 invalid_credentials → inline error.
 *   401 mfa_required → "Two-factor sign-in isn't supported in this dev login yet."
 *   other → generic error message.
 *
 * OAuth callback handling (on mount):
 *   ?token=<one-time> → POST /api/app/oauth/native/exchange → setToken → /today
 *   ?error=<reason>   → friendly inline error message
 *   Both params are stripped from the URL immediately via history.replaceState.
 *
 * Route-level component; raw HTML + Tailwind tokens are acceptable here.
 */
import {
  type FC,
  type FormEvent,
  useState,
  useCallback,
  useEffect,
} from "react";
import { createRoute, useNavigate } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useSignInMutation, useOAuthProvidersQuery, apiClient, setToken } from "~/lib/api";
import type { OAuthProvider } from "~/lib/api";
import { ApiError } from "~/lib/api";
import { cn } from "~/lib/utils";
import { OAuthSignInButton } from "./OAuthSignInButton";

// ── OAuth error message map ───────────────────────────────────────────────────

const OAUTH_ERROR_MESSAGES: Record<string, string> = {
  signup_closed:
    "That account isn't set up yet — you'll need an invite.",
  existing_account:
    "An account with that email already exists — sign in with your password.",
  invalid: "That sign-in link was invalid or expired. Please try again.",
  deletion_requested:
    "This account is scheduled for deletion and can't be used to sign in.",
  mailbox_has_owner:
    "We couldn't complete sign-in. Please try again or contact support.",
  mailbox_no_owner:
    "We couldn't complete sign-in. Please try again or contact support.",
};

const oauthErrorMessage = (reason: string): string =>
  OAUTH_ERROR_MESSAGES[reason] ?? "Sign-in failed. Please try again.";

// ── OAuth token exchange response ─────────────────────────────────────────────

interface OAuthExchangeResponse {
  token: string;
  expires_at: string;
}

// ── LoginPage component ───────────────────────────────────────────────────────

export const LoginPage: FC = () => {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [oauthExchanging, setOauthExchanging] = useState(false);

  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const signIn = useSignInMutation();

  // Fetch enabled OAuth providers (unauthenticated — no bearer needed).
  const { data: providersData } = useOAuthProvidersQuery();
  const providers: OAuthProvider[] = providersData?.providers ?? [];

  // ── OAuth callback handling (on mount) ──────────────────────────────────────
  // Read ?token or ?error from the URL immediately, then strip them from history.

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const oauthToken = params.get("token");
    const oauthError = params.get("error");

    if (!oauthToken && !oauthError) return;

    // Strip the params from the URL immediately so they don't linger.
    window.history.replaceState({}, "", "/login");

    if (oauthError) {
      setErrorMsg(oauthErrorMessage(oauthError));
      return;
    }

    if (oauthToken) {
      setOauthExchanging(true);
      setErrorMsg(null);

      apiClient<OAuthExchangeResponse>("/oauth/native/exchange", {
        method: "POST",
        body: { token: oauthToken },
      })
        .then(async (data) => {
          setToken(data.token);
          await queryClient.invalidateQueries({ queryKey: ["me"] });
          void navigate({ to: "/today", replace: true });
        })
        .catch((err: unknown) => {
          if (err instanceof ApiError) {
            setErrorMsg(err.message || "Sign-in failed. Please try again.");
          } else {
            setErrorMsg("Sign-in failed. Please try again.");
          }
          setOauthExchanging(false);
        });
    }
  }, [navigate, queryClient]);

  // ── Password sign-in ────────────────────────────────────────────────────────

  const handleSubmit = useCallback(
    async (e: FormEvent<HTMLFormElement>): Promise<void> => {
      e.preventDefault();
      setErrorMsg(null);

      try {
        await signIn.mutateAsync({ email_address: email, password });
        await queryClient.invalidateQueries({ queryKey: ["me"] });
        void navigate({ to: "/today", replace: true });
      } catch (err) {
        if (err instanceof ApiError) {
          if (err.code === "mfa_required") {
            setErrorMsg(
              "Two-factor sign-in isn't supported in this dev login yet.",
            );
          } else if (err.code === "invalid_credentials") {
            setErrorMsg("Incorrect email or password.");
          } else {
            setErrorMsg(err.message || "Sign-in failed. Please try again.");
          }
        } else {
          setErrorMsg("Sign-in failed. Please try again.");
        }
      }
    },
    [email, password, signIn, queryClient, navigate],
  );

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="flex min-h-screen items-center justify-center bg-ground px-4">
      <div
        className={cn(
          "w-full max-w-sm rounded-[--cb-r2] border border-line bg-surface-1 p-8",
        )}
      >
        {/* Wordmark */}
        <h1 className="mb-6 text-[20px] font-[650] text-t1 tracking-tight">
          campbooks
        </h1>

        {/* OAuth exchange loading overlay */}
        {oauthExchanging && (
          <div
            className="mb-4 flex items-center gap-2 text-[13px] text-t2"
            data-testid="login.oauth.exchanging"
          >
            <Loader2 size={14} className="animate-spin" aria-hidden="true" />
            Signing you in…
          </div>
        )}

        {/* OAuth sign-in buttons */}
        {!oauthExchanging && providers.length > 0 && (
          <>
            <div className="flex flex-col gap-2 mb-4">
              {providers.map((provider) => (
                <OAuthSignInButton key={provider} provider={provider} />
              ))}
            </div>

            {/* Divider */}
            <div className="relative flex items-center mb-4" aria-hidden="true">
              <div className="flex-1 border-t border-line" />
              <span className="mx-3 text-[11.5px] text-t4 font-[450] select-none">
                or
              </span>
              <div className="flex-1 border-t border-line" />
            </div>
          </>
        )}

        <form onSubmit={(e) => void handleSubmit(e)} noValidate>
          <div className="flex flex-col gap-4">
            {/* Email */}
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="cb-email"
                className="text-[12.5px] font-[500] text-t2"
              >
                Email
              </label>
              <input
                id="cb-email"
                type="email"
                autoComplete="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                disabled={signIn.isPending || oauthExchanging}
                className={cn(
                  "h-9 rounded-cb-1 border border-line bg-ground px-3",
                  "text-[13px] text-t1 placeholder:text-t4 outline-none",
                  "focus:ring-2 focus:ring-[--ring] transition-shadow duration-[--cb-dur]",
                  "disabled:opacity-50 disabled:cursor-not-allowed",
                )}
                placeholder="you@example.com"
                data-testid="login.email"
              />
            </div>

            {/* Password */}
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="cb-password"
                className="text-[12.5px] font-[500] text-t2"
              >
                Password
              </label>
              <input
                id="cb-password"
                type="password"
                autoComplete="current-password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={signIn.isPending || oauthExchanging}
                className={cn(
                  "h-9 rounded-cb-1 border border-line bg-ground px-3",
                  "text-[13px] text-t1 placeholder:text-t4 outline-none",
                  "focus:ring-2 focus:ring-[--ring] transition-shadow duration-[--cb-dur]",
                  "disabled:opacity-50 disabled:cursor-not-allowed",
                )}
                placeholder="••••••••"
                data-testid="login.password"
              />
            </div>

            {/* Inline error (password form or OAuth callback) */}
            {errorMsg != null && (
              <p
                role="alert"
                className="text-[12.5px] text-[--destructive]"
                data-testid="login.oauth.error"
              >
                {errorMsg}
              </p>
            )}

            {/* Submit */}
            <button
              type="submit"
              disabled={signIn.isPending || oauthExchanging || email.length === 0 || password.length === 0}
              className={cn(
                "mt-1 h-9 w-full rounded-cb-1 px-4",
                "bg-[--primary] text-[--primary-foreground] text-[13px] font-[550]",
                "hover:opacity-90 transition-opacity duration-[--cb-dur]",
                "disabled:opacity-40 disabled:cursor-not-allowed",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--ring]",
              )}
              data-testid="login.submit"
            >
              {signIn.isPending ? "Signing in…" : "Sign in"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

// ── Route builder ─────────────────────────────────────────────────────────────

export const buildLoginRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/login",
    component: LoginPage,
  }),
];
