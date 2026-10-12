// @vitest-environment node
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

import { cruise, type IConfiguration, type ICruiseResult } from "dependency-cruiser";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const config = require("../../.dependency-cruiser.cjs") as IConfiguration;
const fixtureRoot = fileURLToPath(new URL("../fixtures/dep-boundary", import.meta.url));

let cached: Promise<{ rule: string; from: string; to: string }[]> | undefined;

/** Cruises the fixture once; every test reads the same result. */
function violations() {
  cached ??= cruiseFixture();
  return cached;
}

async function cruiseFixture() {
  const { output } = await cruise(["src"], {
    baseDir: fixtureRoot,
    validate: true,
    ruleSet: { forbidden: config.forbidden ?? [] },
    tsPreCompilationDeps: true,
  });
  return (output as ICruiseResult).summary.violations.map(({ rule, from, to }) => ({
    rule: rule.name,
    from,
    to,
  }));
}

describe("dependency-cruiser boundaries", () => {
  it("flags a module reaching into another module's internals", async () => {
    expect(await violations()).toContainEqual({
      rule: "module-public-surface",
      from: "src/modules/alpha/index.ts",
      to: "src/modules/beta/internal.ts",
    });
  });

  it("allows imports through a module's index.ts", async () => {
    expect(await violations()).not.toContainEqual(
      expect.objectContaining({
        from: "src/modules/alpha/index.ts",
        to: "src/modules/beta/index.ts",
      }),
    );
  });

  it("flags lib/ importing a module", async () => {
    expect(await violations()).toContainEqual({
      rule: "lib-is-business-agnostic",
      from: "src/lib/helper.ts",
      to: "src/modules/alpha/index.ts",
    });
  });

  it("flags a module reaching past lib/ui's index", async () => {
    expect(await violations()).toContainEqual({
      rule: "ui-public-surface",
      from: "src/modules/gamma/index.ts",
      to: "src/lib/ui/components/thing.ts",
    });
  });

  it("allows lib/ui through its index", async () => {
    expect(await violations()).not.toContainEqual(
      expect.objectContaining({ from: "src/modules/gamma/index.ts", to: "src/lib/ui/index.ts" }),
    );
  });

  it("flags a module importing a stylesheet", async () => {
    expect(await violations()).toContainEqual({
      rule: "modules-have-no-stylesheets",
      from: "src/modules/gamma/index.ts",
      to: "src/modules/gamma/gamma.css",
    });
  });
});
