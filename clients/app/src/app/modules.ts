// The module manifest: every domain module plugs its routes in here, and
// nowhere else. Modules are imported only through their public `index.ts`.
import { buildSystemRoutes } from "~/modules/system";

import type { rootRoute } from "./routes/root";

export function buildModuleRoutes(root: typeof rootRoute) {
  return [...buildSystemRoutes(root)] as const;
}
