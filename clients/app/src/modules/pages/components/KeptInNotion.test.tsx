import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "../../../../test/render";

import { KeptInNotion } from "./KeptInNotion";

describe("KeptInNotion", () => {
  it("says what is kept, that it is not edited here, and where to edit it", () => {
    renderWithProviders(<KeptInNotion kind="column_list" href="https://www.notion.so/abc" />);
    expect(screen.getByText("Kept in Notion · Columns")).toBeVisible();
    expect(
      screen.getByText(/Campbooks shows it but doesn’t edit it yet; it stays exactly as it is/u),
    ).toBeVisible();
    const edit = screen.getByRole("link", { name: "Edit in Notion" });
    expect(edit).toHaveAttribute("href", "https://www.notion.so/abc");
    expect(edit).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("names a kind it has no word for from the kind itself, and links nowhere without a page", () => {
    renderWithProviders(<KeptInNotion kind="some_new_block" />);
    expect(screen.getByText("Kept in Notion · Some new block")).toBeVisible();
    expect(screen.queryByRole("link")).toBeNull();
  });

  it("words the kinds in the reader's language", () => {
    renderWithProviders(<KeptInNotion kind="synced_block" />, "fr");
    expect(screen.getByText("Conservé dans Notion · Bloc synchronisé")).toBeVisible();
  });
});
