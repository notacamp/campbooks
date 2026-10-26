import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";
import { Icon, ICON_NAMES } from "../Icon/Icon";

import { Heading, Text, VisuallyHidden } from "./Typography";

describe("Typography", () => {
  it("keeps the outline level independent of the size", () => {
    renderUi(
      <Heading level={2} size="hero" id="title">
        6 things need you
      </Heading>,
    );

    expect(screen.getByRole("heading", { level: 2 })).toHaveAttribute("data-size", "hero");
  });

  it("renders text with tone, size and options as data attributes", () => {
    renderUi(
      <Text
        as="time"
        tone="danger"
        size="xs"
        weight="semibold"
        numeric
        caps
        truncate
        dateTime="2026-10-12"
      >
        Today
      </Text>,
    );
    const time = screen.getByText("Today");

    expect(time.tagName).toBe("TIME");
    expect(time).toHaveAttribute("data-tone", "danger");
    expect(time).toHaveAttribute("data-numeric", "true");
    expect(time).toHaveAttribute("datetime", "2026-10-12");
  });

  it("hides text visually but not from screen readers", () => {
    renderUi(<VisuallyHidden>Loading</VisuallyHidden>);

    expect(screen.getByText("Loading")).toBeInTheDocument();
  });
});

describe("Icon", () => {
  it.each(ICON_NAMES)("draws %s decoratively", (name) => {
    const { container } = renderUi(<Icon name={name} size="lg" />);

    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
