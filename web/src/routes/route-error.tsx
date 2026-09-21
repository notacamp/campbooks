/**
 * RouteError — default error component for the router.
 * Rendered inside the shell when a route or its loader throws.
 */
import { type FC } from "react";
import { useRouter } from "@tanstack/react-router";

export const RouteError: FC = () => {
  const router = useRouter();
  return (
    <div data-testid="app.routeError.root" style={{ padding: "2rem" }}>
      <h2>Something went wrong</h2>
      <p>An unexpected error occurred while loading this page.</p>
      <button
        type="button"
        onClick={() => router.invalidate()}
        data-testid="app.routeError.retry"
      >
        Try again
      </button>
    </div>
  );
};
