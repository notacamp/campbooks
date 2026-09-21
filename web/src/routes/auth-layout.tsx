/**
 * AuthLayout — protected shell route wrapping all authenticated surfaces.
 *
 * beforeLoad: if no bearer token is stored, throws a redirect to /login.
 * Component: renders AppShell (Sidebar | main + ScoutBar).
 * Watches useMe — if it errors with 401 (expired/revoked token), clears the
 * stored token and navigates to /login.
 *
 * Route ID: "_auth" (pathless layout — does not appear in the URL).
 */
import { type FC, useEffect } from "react";
import { createRoute, redirect, useNavigate } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { AppShell } from "./shell";
import { getToken, clearToken, ApiError, useMe } from "~/lib/api";

// ── AuthLayout component ──────────────────────────────────────────────────────

const AuthLayout: FC = () => {
  const { error } = useMe();
  const navigate = useNavigate();

  // If the stored token is rejected by the server (expired / revoked), sign out.
  useEffect(() => {
    if (error instanceof ApiError && error.status === 401) {
      clearToken();
      void navigate({ to: "/login", replace: true });
    }
  }, [error, navigate]);

  return <AppShell />;
};

// ── Route builder ─────────────────────────────────────────────────────────────

/**
 * Builds the "_auth" pathless layout route as a child of `parent`.
 * All authenticated routes should be nested under this route so the auth
 * guard applies universally.
 */
export const buildAuthRoute = (parent: AnyRoute): AnyRoute =>
  createRoute({
    getParentRoute: () => parent,
    id: "_auth",
    component: AuthLayout,
    beforeLoad: () => {
      // Synchronous token check: redirect immediately if there is no stored token.
      if (!getToken()) {
        throw redirect({ to: "/login" });
      }
    },
  });
