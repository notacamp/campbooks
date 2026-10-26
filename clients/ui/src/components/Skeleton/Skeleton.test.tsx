import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { Skeleton, SkeletonList } from "./Skeleton";

describe("Skeleton", () => {
  it("announces a loading list once and hides the shapes", () => {
    renderUi(<SkeletonList rows={4} />, { locale: "es-ES" });
    const status = screen.getByRole("status");

    expect(status).toHaveAttribute("aria-busy", "true");
    expect(status).toHaveTextContent("Cargando");
  });

  it("sizes a line from its width prop", () => {
    const { container } = renderUi(<Skeleton width="40%" />);

    expect(container.firstElementChild).toHaveStyle({ "--skeleton-width": "40%" });
  });
});
