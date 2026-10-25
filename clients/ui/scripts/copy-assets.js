// After `vite build` and the declarations: puts next to them what the bundles
// don't carry. The stylesheets an app imports as they are (tokens, the
// Tailwind theme, base, fonts), the message bundles the declarations refer
// to (`typeof en`), and the license.
import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const src = path.join(root, "src");
const dist = path.join(root, "dist");

/** @param {string} from @param {string} to */
function copy(from, to) {
  mkdirSync(path.dirname(to), { recursive: true });
  copyFileSync(from, to);
}

copy(path.join(src, "tokens/tokens.css"), path.join(dist, "tokens.css"));
copy(path.join(src, "tokens/tailwind-theme.css"), path.join(dist, "tailwind-theme.css"));
copy(path.join(src, "styles/base.css"), path.join(dist, "base.css"));
copy(path.join(src, "styles/fonts.css"), path.join(dist, "fonts.css"));

for (const folder of ["i18n", "shortcuts/i18n"]) {
  for (const file of readdirSync(path.join(src, folder))) {
    if (file.endsWith(".json")) {
      copy(path.join(src, folder, file), path.join(dist, folder, file));
    }
  }
}

copy(path.join(root, "../../LICENSE"), path.join(dist, "LICENSE"));
