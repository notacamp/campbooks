/**
 * RootLayout — the thin wrapper that every route lives inside.
 *
 * Responsibilities:
 *   - ThemeProvider from next-themes (dark by default, class strategy).
 *   - Outlet renders either the AuthLayout (for authenticated routes) or
 *     the LoginPage (unauthenticated) — the AppShell lives in AuthLayout,
 *     not here, so /login renders without a sidebar.
 *   - TanStack Router devtools in development.
 */
import { type FC } from "react";
import { Outlet } from "@tanstack/react-router";
import { ThemeProvider } from "next-themes";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";

export const RootLayout: FC = () => (
  <ThemeProvider attribute="class" defaultTheme="dark" disableTransitionOnChange>
    <Outlet />
    {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
  </ThemeProvider>
);
