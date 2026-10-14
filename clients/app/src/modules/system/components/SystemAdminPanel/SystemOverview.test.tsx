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
      "Answeringqwen3:8b at http://ollama:11434 · OpenAI-compatible · on this instance’s network · " +
        "got 33 of 33 test answers right",
    );
    expect(valueOf("Embeddings")).toHaveTextContent("Not answering");
    expect(valueOf("Images and scans")).toHaveTextContent("Not set up");
  });

  it("says when the decisions model fails its answers check, and that its answers are not used", async () => {
    const failing = {
      ...OVERVIEW,
      ai: OVERVIEW.ai.map((role) =>
        role.role === "decide"
          ? {
              ...role,
              model: "convaiinnovations/laya-multilingual",
              protocol: "laya" as const,
              answers_check: {
                state: "failed" as const,
                right: 8,
                total: 33,
                checked_at: "2026-10-14T06:00:00Z",
              },
            }
          : role,
      ),
    };
    renderWithProviders(<SystemOverview overview={failing} />);

    expect(await screen.findByText("Decisions")).toBeVisible();
    expect(valueOf("Decisions")).toHaveTextContent(
      "Answers fail the checkconvaiinnovations/laya-multilingual at http://ollama:11434 · Laya · " +
        "on this instance’s network · got only 8 of 33 obvious test answers right, so its " +
        "answers are not used and the rules sort mail alone",
    );
    expect(valueOf("Decisions").querySelector("[data-status='problem']")).not.toBeNull();
  });

  it("says while the first answers check runs", async () => {
    const pending = {
      ...OVERVIEW,
      ai: OVERVIEW.ai.map((role) =>
        role.role === "decide"
          ? {
              ...role,
              answers_check: {
                state: "pending" as const,
                right: null,
                total: null,
                checked_at: null,
              },
            }
          : role,
      ),
    };
    renderWithProviders(<SystemOverview overview={pending} />);

    expect(await screen.findByText("Decisions")).toBeVisible();
    expect(valueOf("Decisions")).toHaveTextContent(/^Checking its answers/u);
    // Not used yet: not shown as healthy.
    expect(valueOf("Decisions").querySelector("[data-status='off']")).not.toBeNull();
    expect(valueOf("Decisions").querySelector("[data-status='ok']")).toBeNull();
  });

  it("says when a role is outside the instance's network and gets no mail", async () => {
    renderWithProviders(<SystemOverview overview={OVERVIEW} />);

    expect(await screen.findByText("Writing")).toBeVisible();
    expect(valueOf("Writing")).toHaveTextContent(
      "Not sent any mailopen-mixtral at https://api.example.com · OpenAI-compatible · " +
        "outside this instance’s network, so mail is not sent to it",
    );
  });

  it("says when an outside role was allowed to receive mail", async () => {
    const allowed = {
      ...OVERVIEW,
      ai: OVERVIEW.ai.map((role) =>
        role.role === "complete" ? { ...role, content_allowed: true } : role,
      ),
    };
    renderWithProviders(<SystemOverview overview={allowed} />);

    expect(await screen.findByText("Writing")).toBeVisible();
    expect(valueOf("Writing")).toHaveTextContent(
      "Answeringopen-mixtral at https://api.example.com · OpenAI-compatible · " +
        "outside this instance’s network, allowed to receive mail",
    );
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
