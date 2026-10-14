import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Highlight } from "./Highlight";

describe("Highlight", () => {
  it("marks the hits and leaves the rest as plain text", () => {
    const { container } = render(
      <p>
        <Highlight
          segments={[
            { text: "Invoice ", hit: false },
            { text: "October", hit: true },
            { text: " is due", hit: false },
          ]}
        />
      </p>,
    );

    expect(container.textContent).toBe("Invoice October is due");
    const marks = container.querySelectorAll("mark");
    expect(marks).toHaveLength(1);
    expect(marks[0]?.textContent).toBe("October");
  });

  it("never turns text into elements", () => {
    const { container } = render(
      <p>
        <Highlight
          segments={[
            { text: "<img src=x onerror=alert(1)>", hit: true },
            { text: "<b>plain</b>", hit: false },
          ]}
        />
      </p>,
    );

    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
    expect(container.textContent).toBe("<img src=x onerror=alert(1)><b>plain</b>");
  });
});
