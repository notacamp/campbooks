import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Chip, ChipLink, ChipRow } from "./Chip";

describe("Chip and ChipLink", () => {
  it("a chip that goes somewhere is a link, the open page marked as such", () => {
    renderUi(
      <ChipRow label="Settings sections">
        <ChipLink href="/members">Members</ChipLink>
        <ChipLink href="/mailboxes" current>
          Mailboxes
        </ChipLink>
      </ChipRow>,
    );

    expect(screen.getByRole("link", { name: "Mailboxes" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Members" })).not.toHaveAttribute("aria-current");
  });

  it("a filter chip is a pressed button", () => {
    renderUi(<Chip pressed>Primary</Chip>);

    expect(screen.getByRole("button", { name: "Primary" })).toHaveAttribute("aria-pressed", "true");
  });
});
