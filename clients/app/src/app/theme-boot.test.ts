import { afterEach, describe, expect, it } from "vitest";

import { PREF_PREFIX, writePref } from "~/lib/storage/prefs";

import bootScript from "../../public/theme-boot.js?raw";

// The boot script applies the saved theme before any module loads, so it
// must read the same keys, under the same prefix, as the ThemeProvider
// writes through `~/lib/storage` (the app passes it as the theme's prefs).
const root = document.documentElement;

function resetPrefs() {
  for (const key of ["theme", "density", "workspace"] as const) {
    writePref(key, "");
  }
  for (const name of ["theme", "accent", "density"]) {
    root.removeAttribute(`data-${name}`);
  }
}

describe("public/theme-boot.js", () => {
  /** Runs the shipped file as the browser does: a classic script element. */
  const boot = () => {
    const script = document.createElement("script");
    script.textContent = bootScript;
    document.head.append(script);
    script.remove();
  };

  afterEach(resetPrefs);

  it("uses the same storage prefix as lib/storage", () => {
    expect(bootScript).toContain(`"${PREF_PREFIX}"`);
  });

  it("applies the saved theme, density and last workspace's accent before React loads", () => {
    writePref("theme", "dark");
    writePref("density", "compact");
    writePref("workspace", "w9");
    writePref("accent.w9", "forest");

    boot();

    expect(root.dataset).toMatchObject({ theme: "dark", accent: "forest", density: "compact" });
  });

  it("falls back to the defaults for missing or unknown values", () => {
    writePref("theme", "neon");

    boot();

    expect(root.dataset).toMatchObject({
      theme: "system",
      accent: "business",
      density: "comfortable",
    });
  });
});
