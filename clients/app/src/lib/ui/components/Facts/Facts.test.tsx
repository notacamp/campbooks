import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Fact, FactList } from "./Facts";

describe("FactList and Fact", () => {
  it("pairs each label with its value, status and detail", () => {
    const { container } = renderUi(
      <FactList>
        <Fact label="Database" status="ok" detail="3 ms">
          Reachable
        </Fact>
        <Fact label="Version">0.1.0</Fact>
      </FactList>,
    );

    const terms = screen.getAllByRole("term");
    const values = screen.getAllByRole("definition");
    expect(terms.map((term) => term.textContent)).toStrictEqual(["Database", "Version"]);
    expect(values[0]).toHaveTextContent("Reachable3 ms");
    expect(container.querySelector("[data-status='ok']")).toHaveAttribute("aria-hidden", "true");
    expect(values[1]?.querySelector("[data-status]")).toBeNull();
  });
});
