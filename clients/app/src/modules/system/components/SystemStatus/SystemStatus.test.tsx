import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders, stubApi } from "../../../../../test/render";

import { SystemStatus } from "./SystemStatus";

describe("SystemStatus", () => {
  it("shows the API version and health", async () => {
    stubApi({
      "/bff/api/v1/system/version": { body: { version: "0.1.0", commit: "abc1234" } },
      "/bff/api/v1/system/health": { body: { status: "ok", database: "ok" } },
    });

    renderWithProviders(<SystemStatus />);

    expect(await screen.findByRole("heading", { name: "System status" })).toBeVisible();
    expect(await screen.findByText("0.1.0 (abc1234)")).toBeVisible();
    expect(await screen.findByText("Service ok, database ok")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it.each([
    ["null", { version: "0.2.0", commit: null }],
    ["absent", { version: "0.2.0" }],
  ])("shows a version whose commit is %s", async (_, body) => {
    stubApi({
      "/bff/api/v1/system/version": { body },
      "/bff/api/v1/system/health": { body: { status: "ok", database: "ok" } },
    });

    renderWithProviders(<SystemStatus />);

    expect(await screen.findByText("0.2.0")).toBeVisible();
  });

  it("shows a graceful error when the API is unreachable", async () => {
    stubApi({});

    renderWithProviders(<SystemStatus />);

    expect(await screen.findByRole("alert")).toHaveTextContent("Can’t reach the Campbooks API");
    expect(screen.getByText("Unreachable")).toBeVisible();
    expect(screen.getByText("Unknown")).toBeVisible();
  });

  it("treats a problem+json response as an error", async () => {
    stubApi({
      "/bff/api/v1/system/version": { body: { version: "0.1.0", commit: null } },
      "/bff/api/v1/system/health": { status: 503, body: { title: "Database unavailable" } },
    });

    renderWithProviders(<SystemStatus />);

    expect(await screen.findByText("Unreachable")).toBeVisible();
    expect(screen.getByText("0.1.0")).toBeVisible();
  });

  it("renders in the user's language", async () => {
    stubApi({});

    renderWithProviders(<SystemStatus />, "pt-PT");

    expect(await screen.findByRole("heading", { name: "Estado do sistema" })).toBeVisible();
  });
});
