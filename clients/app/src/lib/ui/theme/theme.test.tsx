import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PREF_PREFIX, readPref, writePref } from "~/lib/storage/prefs";

import bootScript from "../../../../public/theme-boot.js?raw";

import { applyTheme, parseAccent, parseDensity, parseThemeMode, readStoredTheme } from "./theme";
import { ThemeProvider, useTheme } from "./ThemeProvider";

function Controls() {
  const theme = useTheme();
  return (
    <div>
      <p>{`${theme.theme} ${theme.accent} ${theme.density} ${theme.resolvedTheme}`}</p>
      <button
        type="button"
        onClick={() => {
          theme.setTheme("dark");
        }}
      >
        dark
      </button>
      <button
        type="button"
        onClick={() => {
          theme.setAccent("forest");
        }}
      >
        forest
      </button>
      <button
        type="button"
        onClick={() => {
          theme.setDensity("compact");
        }}
      >
        compact
      </button>
    </div>
  );
}

const root = document.documentElement;

/** Resets the device-wide preferences (workspace accents use a unique id per test). */
function resetPrefs() {
  for (const key of ["theme", "density", "workspace"] as const) {
    writePref(key, "");
  }
  for (const name of ["theme", "accent", "density"]) {
    root.removeAttribute(`data-${name}`);
  }
}

describe("theme settings", () => {
  afterEach(resetPrefs);

  it("parses only known values, defaulting to system, ink and comfortable", () => {
    expect(parseThemeMode("dark")).toBe("dark");
    expect(parseThemeMode("sepia")).toBe("system");
    expect(parseAccent(null)).toBe("business");
    expect(parseAccent("personal")).toBe("personal");
    expect(parseDensity("compact")).toBe("compact");
    expect(parseDensity("tiny")).toBe("comfortable");
  });

  it("reads a workspace's accent and the device's theme and density", () => {
    writePref("theme", "light");
    writePref("density", "compact");
    writePref("accent.read-1", "forest");

    expect(readStoredTheme("read-1")).toEqual({
      theme: "light",
      accent: "forest",
      density: "compact",
    });
    expect(readStoredTheme("read-2").accent).toBe("business");
  });

  it("writes the settings as data attributes", () => {
    const element = document.createElement("div");
    applyTheme({ theme: "dark", accent: "personal", density: "compact" }, element);

    expect(element.dataset).toMatchObject({
      theme: "dark",
      accent: "personal",
      density: "compact",
    });
  });

  it("applies, changes and remembers the theme on <html>", () => {
    render(
      <ThemeProvider workspaceId="apply-1">
        <Controls />
      </ThemeProvider>,
    );
    expect(root.dataset).toMatchObject({
      theme: "system",
      accent: "business",
      density: "comfortable",
    });
    expect(screen.getByRole("paragraph")).toHaveTextContent("system business comfortable light");

    act(() => {
      screen.getByRole("button", { name: "dark" }).click();
      screen.getByRole("button", { name: "forest" }).click();
      screen.getByRole("button", { name: "compact" }).click();
    });

    expect(root.dataset).toMatchObject({ theme: "dark", accent: "forest", density: "compact" });
    expect(readPref("theme")).toBe("dark");
    expect(readPref("accent.apply-1")).toBe("forest");
    expect(readPref("density")).toBe("compact");
    expect(readPref("workspace")).toBe("apply-1");
  });

  it("swaps in the next workspace's own accent", () => {
    writePref("accent.swap-2", "personal");
    const { rerender } = render(
      <ThemeProvider workspaceId="swap-1">
        <Controls />
      </ThemeProvider>,
    );
    rerender(
      <ThemeProvider workspaceId="swap-2">
        <Controls />
      </ThemeProvider>,
    );

    expect(root.dataset["accent"]).toBe("personal");
  });

  it("resolves system to the OS scheme when matchMedia exists", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
    );
    render(
      <ThemeProvider>
        <Controls />
      </ThemeProvider>,
    );

    expect(screen.getByRole("paragraph")).toHaveTextContent("dark");
  });

  it("refuses useTheme outside a provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => {
      // React logs the error boundary miss; the assertion below is what matters.
    });
    expect(() => render(<Controls />)).toThrow(/inside <ThemeProvider>/);
  });
});

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
