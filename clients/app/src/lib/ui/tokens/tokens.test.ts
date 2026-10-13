import { describe, expect, it } from "vitest";

import tailwindTheme from "./tailwind-theme.css?raw";
import tokensCss from "./tokens.css?raw";
import { ACCENTS, COMPONENT_TOKENS, cssVar, SEMANTIC_TOKENS } from "./tokens";

function declared(css: string): Set<string> {
  return new Set([...css.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1] ?? ""));
}

describe("design tokens", () => {
  const names = declared(tokensCss);

  it("declares every semantic and component token listed in tokens.ts", () => {
    const listed = [...Object.values(SEMANTIC_TOKENS).flat(), ...COMPONENT_TOKENS];
    const missing = listed.filter((name) => !names.has(`--${name}`));

    expect(missing).toEqual([]);
  });

  it("defines every colour role for light and dark with light-dark()", () => {
    for (const name of ["surface-base", "text-primary", "border-subtle", "danger", "accent"]) {
      const declaration = new RegExp(String.raw`--${name}:\s*light-dark\(`).exec(tokensCss);

      expect(declaration, `--${name}`).not.toBeNull();
    }
  });

  it("has a preset for every accent, and ink (business) carries no hue of its own", () => {
    for (const accent of ACCENTS) {
      expect(tokensCss).toContain(`[data-accent="${accent}"]`);
    }
    expect(/--accent-c:\s*0\.008;/.test(tokensCss)).toBe(true);
  });

  it("maps Tailwind utilities only onto declared tokens", () => {
    const referenced = [...tailwindTheme.matchAll(/var\((--[\w-]+)\)/g)].map((match) => match[1]);
    const unknown = referenced.filter((name) => name !== undefined && !names.has(name));

    expect(unknown).toEqual([]);
    expect(tailwindTheme).toContain("--*: initial;");
  });

  it("builds a typed var() reference", () => {
    expect(cssVar("surface-raised")).toBe("var(--surface-raised)");
  });
});
