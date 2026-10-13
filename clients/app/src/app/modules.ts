// The module manifest: every domain module plugs its routes in here, and
// nowhere else. Modules are imported only through their public `index.ts`.
import { buildSystemRoutes } from "~/modules/system";
import { buildConversationsRoutes } from "~/modules/conversations";

import type { rootRoute } from "./routes/root";

export function buildModuleRoutes(root: typeof rootRoute) {
  return [
    ...buildSystemRoutes(root),
    ...buildConversationsRoutes(root),
    // `cargo xtask new-module` inserts above this line.
  ] as const;
}
