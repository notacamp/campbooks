import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DEFAULT_SHORTCUTS } from "./catalog";
import { ShortcutRegistry, useShortcutDefinition, useShortcuts } from "./index";
import { ShortcutProvider } from "./ShortcutProvider";

function Probe() {
  const { registry } = useShortcuts();
  const definition = useShortcutDefinition("nav.inbox");
  return <p>{`${definition?.keys ?? "none"} ${String(registry.definitions().length)}`}</p>;
}

describe("ShortcutProvider", () => {
  it("defaults to Campbooks' catalogue", () => {
    render(
      <ShortcutProvider platform="other">
        <Probe />
      </ShortcutProvider>,
    );

    expect(screen.getByRole("paragraph")).toHaveTextContent(
      `g i ${String(DEFAULT_SHORTCUTS.length)}`,
    );
  });

  it("uses the registry it is given", () => {
    render(
      <ShortcutProvider registry={new ShortcutRegistry([])} platform="other">
        <Probe />
      </ShortcutProvider>,
    );

    expect(screen.getByRole("paragraph")).toHaveTextContent("none 0");
  });
});
