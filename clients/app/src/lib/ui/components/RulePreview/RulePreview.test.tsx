import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { RulePreview } from "./RulePreview";

describe("RulePreview", () => {
  it("shows the sentence, what it matches and a few of those emails", () => {
    renderUi(
      <RulePreview
        tag="Rule"
        sentence="Newsletters never need you."
        summary="Matches 312 emails"
        samplesLabel="Emails it matches"
        samples={[
          { subject: "Weekly digest", sender: "Archdaily", when: "14 Oct" },
          { subject: "<b>Daily</b>", sender: "Dezeen", when: "13 Oct" },
        ]}
      />,
    );

    expect(screen.getByText("Newsletters never need you.")).toBeVisible();
    expect(screen.getByText("Matches 312 emails")).toBeVisible();
    const list = screen.getByRole("list", { name: "Emails it matches" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    // Mail text is plain text, never markup.
    expect(within(list).getByText("<b>Daily</b>")).toBeVisible();
  });

  it("holds the summary's place while counting", () => {
    renderUi(<RulePreview sentence="Newsletters never need you." pending />);

    expect(screen.queryByRole("list")).toBeNull();
  });
});
