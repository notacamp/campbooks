/**
 * NotFound — rendered when no route matches the current URL.
 * Displayed inside the app shell once the shell is built.
 */
import { type FC } from "react";

export const NotFound: FC = () => (
  <div data-testid="app.notFound.root" style={{ padding: "2rem" }}>
    <h2>404 — Page not found</h2>
    <p>The page you are looking for does not exist.</p>
  </div>
);
