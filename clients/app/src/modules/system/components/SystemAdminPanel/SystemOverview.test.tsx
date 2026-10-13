import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { OVERVIEW, OVERVIEW_WITHOUT_AI } from "../../../../../test/demo/admin";
import { renderWithProviders } from "../../../../../test/render";

import { formatUptime, SystemOverview } from "./SystemOverview";

/** The value next to `label`. */
function valueOf(label: string) {
  const term = screen.getByText(label, { selector: "dt" });
  const row = term.parentElement;
  if (row === null) {
    throw new Error(`no row for ${label}`);
  }
  return within(row).getByRole("definition");
}

describe("SystemOverview", () => {
  it("shows the build, the database, the configuration and the services in words", async () => {
    renderWithProviders(<SystemOverview overview={OVERVIEW} />);

    expect(await screen.findByRole("heading", { name: "This server" })).toBeVisible();
    expect(valueOf("Version")).toHaveTextContent("0.1.0");
    expect(valueOf("Build")).toHaveTextContent("3f2c1e0");
    expect(valueOf("Running for")).toHaveTextContent("2 days, 5 hours");
    expect(valueOf("Connection")).toHaveTextContent("AnsweringAnswered in 3 ms");
    expect(valueOf("Migrations applied")).toHaveTextContent("18");
    expect(valueOf("Who can sign up")).toHaveTextContent("After an administrator approves");
    expect(valueOf("File storage")).toHaveTextContent("S3-compatible bucket");
    expect(valueOf("Database logins")).toHaveTextContent("Separate runtime and migration logins");
    expect(valueOf("Traces and metrics")).toHaveTextContent("Off");
    expect(valueOf("Sign-in service")).toHaveTextContent("Campbooks’ own");
    expect(valueOf("Mailboxes from")).toHaveTextContent("Google");
  });

  it("says which AI roles answer, and which are not set up", async () => {
    renderWithProviders(<SystemOverview overview={OVERVIEW} />);

    expect(await screen.findByText("Decisions")).toBeVisible();
    expect(valueOf("Decisions")).toHaveTextContent(
      "Answeringqwen3:8b at http://ollama:11434 · OpenAI-compatible",
    );
    expect(valueOf("Embeddings")).toHaveTextContent("Not answering");
    expect(valueOf("Writing")).toHaveTextContent("Not set up");
    expect(valueOf("Images and scans")).toHaveTextContent("Not set up");
  });

  it("works with no AI at all", async () => {
    renderWithProviders(<SystemOverview overview={OVERVIEW_WITHOUT_AI} />);

    expect(await screen.findByText("Decisions")).toBeVisible();
    expect(screen.getAllByText("Not set up")).toHaveLength(4);
  });
});

describe("formatUptime", () => {
  it("names the two largest units", () => {
    expect(formatUptime(3 * 86_400 + 4 * 3600 + 5 * 60, "en")).toBe("3 days, 4 hours");
    expect(formatUptime(3600 + 60, "en")).toBe("1 hour, 1 minute");
    expect(formatUptime(30, "en")).toBeUndefined();
    expect(formatUptime(2 * 3600, "pt-PT")).toBe("2 horas");
  });
});
