/**
 * LoginPage — sign-in form at /login.
 *
 * POST /api/app/session { email_address, password }
 *   200 → setToken, invalidate ["me"], navigate to /today.
 *   401 invalid_credentials → inline error.
 *   401 mfa_required → "Two-factor sign-in isn't supported in this dev login yet."
 *   other → generic error message.
 *
 * Route-level component; raw HTML + Tailwind tokens are acceptable here.
 */
import { type FC, type FormEvent, useState, useCallback } from "react";
import { createRoute, useNavigate } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useSignInMutation } from "~/lib/api/hooks/use-sign-in";
import { ApiError } from "~/lib/api";
import { cn } from "~/lib/utils";

// ── LoginPage component ───────────────────────────────────────────────────────

export const LoginPage: FC = () => {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const signIn = useSignInMutation();

  const handleSubmit = useCallback(
    async (e: FormEvent<HTMLFormElement>): Promise<void> => {
      e.preventDefault();
      setErrorMsg(null);

      try {
        await signIn.mutateAsync({ email_address: email, password });
        // Invalidate the bootstrap query so the shell reflects the new session.
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
                disabled={signIn.isPending}
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
                disabled={signIn.isPending}
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

            {/* Inline error */}
            {errorMsg != null && (
              <p
                role="alert"
                className="text-[12.5px] text-[--destructive]"
                data-testid="login.error"
              >
                {errorMsg}
              </p>
            )}

            {/* Submit */}
            <button
              type="submit"
              disabled={signIn.isPending || email.length === 0 || password.length === 0}
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
