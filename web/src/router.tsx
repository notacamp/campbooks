/**
 * THE router — one createRouter for the whole app, here and nowhere else.
 *
 * Architecture rules (enforced by ESLint + dependency-cruiser):
 *   - createRouter and createRootRoute may only be called in THIS file.
 *   - createRoute may be called here and in each module's routes.tsx.
 *   - Route definitions stay thin: path + component. No JSX, no loaders here.
 *     Markup lives in Page components; fetching lives in module api/ hooks.
 *
 * Auth layout:
 *   /login          → unauthenticated, no shell (LoginPage)
 *   /               → redirects to /today (or /login if no token)
 *   /_auth/*        → protected layout (AppShell + beforeLoad token check)
 *     /today, /inbox, /books, /calendar
 */
import { createRootRoute, createRoute, createRouter, redirect } from "@tanstack/react-router";
import { RootLayout } from "./routes/root-layout";
import { NotFound } from "./routes/not-found";
import { RouteError } from "./routes/route-error";
import { buildLoginRoutes } from "./routes/login";
import { buildAuthRoute } from "./routes/auth-layout";
import { buildInboxRoutes } from "./routes/inbox";
import { buildTodayRoutes } from "./routes/today";
import { buildBooksRoutes } from "./routes/books";
import { buildCalendarRoutes } from "./routes/calendar";

// ── Root route ────────────────────────────────────────────────────────────────
// Renders RootLayout (ThemeProvider + Outlet) around every child via <Outlet />.

const rootRoute = createRootRoute({
  component: RootLayout,
  notFoundComponent: NotFound,
  errorComponent: RouteError,
});

// ── Root redirect (/ → /today) ────────────────────────────────────────────────
// The _auth beforeLoad will catch the unauthenticated case and redirect to /login.
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: "/today" });
  },
});

// ── Auth layout route (pathless, wraps all protected surfaces) ────────────────
const authRoute = buildAuthRoute(rootRoute);

// ── Route tree ────────────────────────────────────────────────────────────────

const routeTree = rootRoute.addChildren([
  indexRoute,
  ...buildLoginRoutes(rootRoute),
  authRoute.addChildren([
    ...buildTodayRoutes(authRoute),
    ...buildInboxRoutes(authRoute),
    ...buildBooksRoutes(authRoute),
    ...buildCalendarRoutes(authRoute),
  ]),
]);

// ── Router ────────────────────────────────────────────────────────────────────

export const router = createRouter({
  routeTree,
  defaultErrorComponent: RouteError,
  defaultNotFoundComponent: NotFound,
  // Hover/touch prefetch — starts loading the next page's chunk early so the
  // navigation feels instant even on a slow connection.
  defaultPreload: "intent",
  defaultPreloadDelay: 50,
  // Restore scroll position when navigating back to a route.
  scrollRestoration: true,
});

// Register the router instance with TanStack Router's type system.
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
