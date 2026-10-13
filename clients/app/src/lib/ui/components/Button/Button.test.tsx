import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Button, ButtonLink } from "./Button";

describe("Button", () => {
  it("is a real button that shows its shortcut from the registry", () => {
    const onClick = vi.fn();
    renderUi(
      <Button variant="primary" shortcut="command.open" onClick={onClick}>
        Search
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Search" });

    fireEvent.click(button);

    expect(onClick).toHaveBeenCalledOnce();
    expect(button).toHaveAttribute("type", "button");
    expect(button).toHaveAttribute("data-variant", "primary");
    expect(button).toHaveAttribute("aria-keyshortcuts", "Control+K");
    expect(button).toHaveTextContent("SearchCtrlK");
  });

  it("shows one-off keys and keeps an explicit aria-keyshortcuts", () => {
    renderUi(
      <Button keys="1" aria-keyshortcuts="Digit1">
        Primary
      </Button>,
    );

    expect(screen.getByRole("button")).toHaveAttribute("aria-keyshortcuts", "Digit1");
    expect(screen.getByRole("button")).toHaveTextContent("Primary1");
  });

  it("renders a touch-size link with an icon", () => {
    renderUi(
      <ButtonLink href="#home" size="lg" icon="chevron-left" shortcut="nav.home" grow>
        Home
      </ButtonLink>,
    );
    const link = screen.getByRole("link", { name: /Home/ });

    expect(link).toHaveAttribute("href", "#home");
    expect(link).toHaveAttribute("data-size", "lg");
    expect(link).toHaveAttribute("data-grow", "true");
    expect(link).not.toHaveAttribute("aria-keyshortcuts");
  });
});
