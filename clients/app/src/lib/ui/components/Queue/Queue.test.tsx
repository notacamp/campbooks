import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import {
  AsideSection,
  Figure,
  FigureRow,
  HomeLayout,
  Line,
  LineMeta,
  PageHero,
  QueueGroup,
} from "./Queue";

describe("Queue (home)", () => {
  it("puts the queue in main and the context in a named aside", () => {
    renderUi(
      <HomeLayout
        asideLabel="Context"
        aside={
          <AsideSection title="Money">
            <FigureRow>
              <Figure value="€460.40" label="2 bills due this week" />
            </FigureRow>
            <Line time="09:00" trailing={<LineMeta>Focus</LineMeta>}>
              Held: Q4 review
            </Line>
            <Line undone>Moved 2 newsletters</Line>
            <Line href="#inbox">Primary</Line>
          </AsideSection>
        }
      >
        <PageHero
          title="6 things need you"
          dateLine="Monday 12 October"
          subline="48 sorted"
          visor="alert"
        />
        <QueueGroup signal="reply" name="Reply" count={2}>
          <p>items</p>
        </QueueGroup>
      </HomeLayout>,
    );

    expect(screen.getByRole("main")).toHaveTextContent("6 things need you");
    expect(
      screen.getByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "Reply 2" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "Context" })).toHaveTextContent("€460.40");
    expect(screen.getByText("Moved 2 newsletters").parentElement).toHaveAttribute(
      "data-undone",
      "true",
    );
    expect(screen.getByRole("link", { name: "Primary" })).toHaveAttribute("href", "#inbox");
  });

  it("puts a group's action in its heading row and its note under it", () => {
    renderUi(
      <QueueGroup
        signal="pay"
        name="Pay"
        count={2}
        note="Campbooks can’t see your bank."
        action={<button type="button">All 2 already paid</button>}
      >
        <p>items</p>
      </QueueGroup>,
    );
    expect(screen.getByRole("heading", { level: 2, name: "Pay 2" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "All 2 already paid" })).toBeInTheDocument();
    expect(screen.getByText("Campbooks can’t see your bank.")).toBeInTheDocument();
  });

  it("heads a band with its name and count, Now in the danger tone, with no dot", () => {
    const { container } = renderUi(
      <QueueGroup tone="danger" name="Now" count={2}>
        <p>items</p>
      </QueueGroup>,
    );
    const heading = screen.getByRole("heading", { level: 2, name: "Now 2" });

    expect(heading).toHaveAttribute("data-tone", "danger");
    expect(container.querySelector("[data-signal]")).toBeNull();
  });

  it("drops the visor in the compact hero", () => {
    const { container } = renderUi(<PageHero compact title="5 things need you" />);

    expect(container.querySelector("[data-state]")).toBeNull();
  });
});
