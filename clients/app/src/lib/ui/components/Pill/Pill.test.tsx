import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";
import { Dot } from "../Dot/Dot";
import { Tag } from "../Tag/Tag";

import { Pill } from "./Pill";

describe("Pill, Tag and Dot", () => {
  it("names a signal in the user's language", () => {
    renderUi(<Pill signal="decide" />, { locale: "fr" });

    expect(screen.getByText("Décider")).toHaveAttribute("data-signal", "decide");
  });

  it("takes a custom label", () => {
    renderUi(<Pill signal="pay">Due Friday</Pill>);

    expect(screen.getByText("Due Friday")).toBeInTheDocument();
  });

  it("renders tags with an optional icon and decorative dots", () => {
    const { container } = renderUi(
      <>
        <Tag icon="attachment">CH-20418.pdf</Tag>
        <Dot signal="pay" />
      </>,
    );

    expect(screen.getByText("CH-20418.pdf")).toBeInTheDocument();
    expect(container.querySelector("[data-signal='pay']")).toHaveAttribute("aria-hidden", "true");
  });
});
