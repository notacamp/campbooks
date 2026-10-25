import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";
import { EmptyState } from "../EmptyState/EmptyState";

import { Banner } from "./Banner";

describe("Banner and EmptyState", () => {
  it("leads with a bold title and offers one action", () => {
    renderUi(
      <Banner
        tone="danger"
        role="alert"
        title="Mailbox stopped syncing."
        action={<button type="button">Reconnect</button>}
      >
        Sign in again.
      </Banner>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Mailbox stopped syncing. Sign in again.");
    expect(screen.getByRole("button", { name: "Reconnect" })).toBeInTheDocument();
  });

  it("titles an empty place", () => {
    renderUi(
      <EmptyState
        title="Nothing needs you"
        headingLevel={3}
        action={<button type="button">Edit the rule</button>}
      >
        Campbooks keeps reading.
      </EmptyState>,
    );

    expect(
      screen.getByRole("heading", { level: 3, name: "Nothing needs you" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Campbooks keeps reading.")).toBeInTheDocument();
  });
});
