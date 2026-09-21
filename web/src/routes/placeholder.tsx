/**
 * PlaceholderPage — smoke-test route that confirms:
 *   1. The router, QueryClient, and ThemeProvider are wired correctly.
 *   2. useMe() reaches /api/app/me; a 401 (no token) renders "Not signed in".
 *   3. The CSS token system (Tailwind utilities from tokens.css) renders.
 *
 * This route is the single placeholder wired in router.tsx. Real surfaces
 * (Now, People, Paper, Money, Time, Scout, Settings, Auth) are built in
 * src/modules/ once the design session and module sessions are ready.
 */
import { type FC } from "react";
import { useMe } from "~/lib/api";
import { ApiError } from "~/lib/api";

export const PlaceholderPage: FC = () => {
  const { data: me, error, isPending } = useMe();

  if (isPending) {
    return (
      <div
        data-testid="app.placeholder.loading"
        className="flex items-center justify-center h-screen text-t2"
      >
        Loading…
      </div>
    );
  }

  if (error instanceof ApiError && error.status === 401) {
    return (
      <div
        data-testid="app.placeholder.notSignedIn"
        className="flex flex-col items-center justify-center h-screen gap-4 text-t2"
      >
        <p className="text-lg font-medium text-t1">Not signed in</p>
        <p className="text-sm text-t3">
          POST /api/app/session to obtain a bearer token, then reload.
        </p>
      </div>
    );
  }

  if (error) {
    return (
      <div
        data-testid="app.placeholder.error"
        className="flex items-center justify-center h-screen text-danger"
      >
        Error: {error.message}
      </div>
    );
  }

  return (
    <div
      data-testid="app.placeholder.root"
      className="flex flex-col items-center justify-center h-screen gap-2 bg-ground"
    >
      <p className="text-t1 font-semibold">Campbooks</p>
      {me?.user && (
        <p data-testid="app.placeholder.user" className="text-t2 text-sm">
          {me.user.email}
        </p>
      )}
    </div>
  );
};
