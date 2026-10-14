import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  FAILED_JOBS,
  FAILED_JOBS_FIRST_PAGE,
  JOBS_SUMMARY,
  JOBS_SUMMARY_EMPTY,
} from "../../../../../test/demo/admin";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { JobsAdminPanel } from "./JobsAdminPanel";

const SUMMARY = "GET /bff/admin/v1/system/jobs";
const FAILED = "GET /bff/admin/v1/system/jobs/failed";
/** The first of several matches. */
function first<T>(items: readonly T[]): T {
  const [item] = items;
  if (item === undefined) {
    throw new Error("expected at least one match");
  }
  return item;
}

const ID = FAILED_JOBS.items[0]?.id ?? "";

describe("JobsAdminPanel", () => {
  it("shows the queue by kind, in words, and the failures with their error", async () => {
    mockApi({ [SUMMARY]: { body: JOBS_SUMMARY }, [FAILED]: { body: FAILED_JOBS } });

    renderWithProviders(<JobsAdminPanel />);

    expect(await screen.findByRole("heading", { level: 1, name: "Jobs" })).toBeVisible();
    const queue = await screen.findByRole("list", { name: "Queue" });
    const rows = within(queue).getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("conversations.read_message");
    expect(rows[0]).toHaveTextContent("Ready 12 · Scheduled 3 · Running 2 · Failed 2");
    expect(rows[0]).toHaveTextContent("oldest waiting 3 minutes");
    expect(rows[2]).toHaveTextContent("1 running job lost its worker");

    const failed = await screen.findByRole("list", { name: "Failed jobs" });
    expect(within(failed).getAllByRole("listitem")).toHaveLength(2);
    expect(within(failed).getByLabelText("Last error", { selector: "pre" })).toHaveTextContent(
      "model returned an invalid decision",
    );
    expect(within(failed).getByText("No error was recorded.")).toBeVisible();
  });

  it("retries one failed job and says so", async () => {
    const api = mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY },
      [FAILED]: { body: FAILED_JOBS },
      [`POST /bff/admin/v1/system/jobs/${ID}/retry`]: { body: { outcome: "applied" } },
    });

    renderWithProviders(<JobsAdminPanel />);
    const failed = await screen.findByRole("list", { name: "Failed jobs" });
    fireEvent.click(first(within(failed).getAllByRole("button", { name: "Retry" })));

    expect(await screen.findByText("The job is back in the queue.")).toBeVisible();
    expect(api.calls.map((call) => call.route)).toContain(
      `POST /bff/admin/v1/system/jobs/${ID}/retry`,
    );
  });

  it("asks before discarding, and discards only when confirmed", async () => {
    const api = mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY },
      [FAILED]: { body: FAILED_JOBS },
      [`DELETE /bff/admin/v1/system/jobs/${ID}`]: { body: { outcome: "applied" } },
    });

    renderWithProviders(<JobsAdminPanel />);
    const failed = await screen.findByRole("list", { name: "Failed jobs" });
    fireEvent.click(first(within(failed).getAllByRole("button", { name: "Discard" })));
    expect(await screen.findByText(/It is deleted for good/u)).toBeVisible();
    expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(false);

    fireEvent.click(within(screen.getByRole("group")).getByRole("button", { name: "Discard" }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(true);
    });
    expect(await screen.findByText("The job was discarded.")).toBeVisible();
  });

  it("retries every failed job of a kind after a confirmation", async () => {
    const api = mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY },
      [FAILED]: { body: FAILED_JOBS },
      "POST /bff/admin/v1/system/jobs/retry": { body: { requeued: 2, more: false } },
    });

    renderWithProviders(<JobsAdminPanel />);
    const queue = await screen.findByRole("list", { name: "Queue" });
    fireEvent.click(within(queue).getByRole("button", { name: "Retry all failed" }));
    expect(
      await screen.findByText(/Retry all 2 failed conversations.read_message jobs/u),
    ).toBeVisible();
    fireEvent.click(within(screen.getByRole("group")).getByRole("button", { name: "Retry" }));

    expect(
      await screen.findByText("2 conversations.read_message jobs are back in the queue."),
    ).toBeVisible();
    const call = api.calls.find((entry) => entry.route === "POST /bff/admin/v1/system/jobs/retry");
    expect(call?.body).toStrictEqual({ kind: "conversations.read_message" });
  });

  it("filters the failures by kind and pages with Show more", async () => {
    const api = mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY },
      [FAILED]: (request) => {
        const url = new URL(request.url);
        return {
          body: url.searchParams.get("after") === null ? FAILED_JOBS_FIRST_PAGE : FAILED_JOBS,
        };
      },
    });

    renderWithProviders(<JobsAdminPanel />);
    const queue = await screen.findByRole("list", { name: "Queue" });
    fireEvent.click(within(queue).getByRole("button", { name: "Show failures" }));
    await waitFor(() => {
      expect(screen.getByRole("combobox", { name: "Show" })).toHaveValue(
        "conversations.read_message",
      );
    });
    fireEvent.click(await screen.findByRole("button", { name: "Show more" }));

    await waitFor(() => {
      expect(
        within(screen.getByRole("list", { name: "Failed jobs" })).getAllByRole("listitem"),
      ).toHaveLength(3);
    });
    expect(api.calls.some((call) => call.route === FAILED)).toBe(true);
  });

  it("says plainly when nothing is queued or failed", async () => {
    mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY_EMPTY },
      [FAILED]: { body: { items: [], next: null } },
    });

    renderWithProviders(<JobsAdminPanel />);

    expect(await screen.findByRole("heading", { name: "Nothing is queued" })).toBeVisible();
    expect(await screen.findByRole("heading", { name: "No failed jobs" })).toBeVisible();
  });

  it("offers to try again when the queue cannot be loaded", async () => {
    mockApi({ [SUMMARY]: problem(503, "database_unavailable"), [FAILED]: { body: FAILED_JOBS } });

    renderWithProviders(<JobsAdminPanel />);

    expect(
      await screen.findByRole("heading", { name: "Couldn’t load the job queue" }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
  });

  it("tells the person when a retry fails", async () => {
    mockApi({
      [SUMMARY]: { body: JOBS_SUMMARY },
      [FAILED]: { body: FAILED_JOBS },
      [`POST /bff/admin/v1/system/jobs/${ID}/retry`]: problem(404, "not_found"),
    });

    renderWithProviders(<JobsAdminPanel />);
    const failed = await screen.findByRole("list", { name: "Failed jobs" });
    fireEvent.click(first(within(failed).getAllByRole("button", { name: "Retry" })));

    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t retry the job");
  });
});
