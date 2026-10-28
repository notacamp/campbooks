import { describe, expect, it } from "vitest";

import { literalLinks, shownLinks } from "./dialog";

describe("literalLinks", () => {
  // Scout's draft, saved as a body: no link whose words hide where it goes.
  it("keeps brackets as text, and escapes already there", () => {
    expect(literalLinks(String.raw`Pay [here](https://example.net) or \[keep] C:\temp`)).toBe(
      String.raw`Pay \[here\](https://example.net) or \[keep\] C:\temp`,
    );
  });

  it("shows the dialog's text again as it was", () => {
    const text = "Pay [here](https://example.net) today.";
    expect(shownLinks(literalLinks(text))).toBe(text);
  });
});
