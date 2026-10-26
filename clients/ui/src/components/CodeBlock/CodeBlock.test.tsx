import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { CodeBlock } from "./CodeBlock";

const props = {
  label: "Last error",
  copyLabel: "Copy error",
  copiedLabel: "Copied.",
  failedLabel: "Couldn’t copy.",
};

describe("CodeBlock", () => {
  it("shows the text as written and copies it", async () => {
    const copy = vi.fn(() => Promise.resolve());
    renderUi(
      <CodeBlock {...props} copy={copy}>
        {"line one\n  line two"}
      </CodeBlock>,
    );

    expect(screen.getByLabelText("Last error")).toHaveTextContent("line one line two");
    fireEvent.click(screen.getByRole("button", { name: "Copy error" }));

    expect(await screen.findByText("Copied.")).toBeVisible();
    expect(copy).toHaveBeenCalledWith("line one\n  line two");
  });

  it("says so when the clipboard refuses", async () => {
    renderUi(
      <CodeBlock {...props} copy={() => Promise.reject(new Error("denied"))}>
        boom
      </CodeBlock>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Copy error" }));

    expect(await screen.findByText("Couldn’t copy.")).toBeVisible();
  });
});
