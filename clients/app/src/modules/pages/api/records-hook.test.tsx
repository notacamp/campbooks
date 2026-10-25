import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MARTA, MARTA_DETAIL } from "../../../../test/demo/attention";
import { W } from "../../../../test/demo/conversations";
import { HANDBOOK } from "../../../../test/demo/pages";
import { LEASE, TEAM } from "../../../../test/demo/tasks";
import { DUARTE, RUI } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";
import type { MentionType } from "../types";

import { useRecordLabel } from "./records";

function Probe({ type, id }: { type: MentionType; id: string }) {
  const label = useRecordLabel(DUARTE.id, type, id);
  return (
    <p>
      {label.state}|{label.label}|{label.href ?? ""}
    </p>
  );
}

const routes = {
  [`GET ${W}/pages`]: { body: { pages: [HANDBOOK] } },
  [`GET ${W}/members`]: { body: { members: TEAM } },
  [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
  [`GET ${W}/tasks/${LEASE.id}`]: { body: { task: LEASE } },
  [`GET ${W}/schedule/events/e1`]: { body: { id: "e1", title: "Kickoff lunch" } },
  [`GET ${W}/correspondents`]: {
    body: {
      correspondents: [
        { address: "marta@example.com", name: "Marta Ferreira", wrote: 1, received: 1 },
      ],
    },
  },
  [`GET ${W}/tasks/gone`]: { status: 404, body: { status: 404, title: "Not found" } },
};

const shown = async (type: MentionType, id: string) => {
  mockApi(routes);
  renderWithProviders(<Probe type={type} id={id} />);
  return (await screen.findByText(/^(ready|closed)\|/u)).textContent;
};

describe("what a mention shows", () => {
  it("is the person's name from the workspace", async () => {
    expect(await shown("workspaces.member", RUI.user_id)).toBe("ready|Rui Matos|");
  });

  it("is closed for a member who is not there", async () => {
    expect(await shown("workspaces.member", "nobody")).toBe("closed||");
  });

  it("is a correspondent's name, found by the address", async () => {
    expect(await shown("person", "marta@example.com")).toContain("ready|Marta Ferreira|");
  });

  it("is a conversation's subject", async () => {
    expect(await shown("conversations.conversation", MARTA.id)).toContain(
      `ready|${MARTA.subject}|`,
    );
  });

  it("is a task's title", async () => {
    expect(await shown("tasks.task", LEASE.id)).toBe(
      `ready|${LEASE.title}|/w/${DUARTE.id}/tasks/${LEASE.id}`,
    );
  });

  it("is an event's title", async () => {
    expect(await shown("schedule.event", "e1")).toContain("ready|Kickoff lunch|");
  });

  it("is a page's title, taken from the tree without reading the page", async () => {
    const mocked = mockApi(routes);
    renderWithProviders(<Probe type="pages.page" id={HANDBOOK.id} />);
    expect((await screen.findByText(/^ready\|/u)).textContent).toBe(
      `ready|${HANDBOOK.title}|/w/${DUARTE.id}/pages/${HANDBOOK.id}`,
    );
    expect(mocked.calls.map((call) => call.route)).not.toContain(`GET ${W}/pages/${HANDBOOK.id}`);
  });

  it("is closed for a page that is not in the tree", async () => {
    expect(await shown("pages.page", "0192f1a0-0000-7000-8000-0000000009ff")).toBe("closed||");
  });

  it("says only that a record cannot be opened when the answer is not found", async () => {
    expect(await shown("tasks.task", "gone")).toBe("closed||");
  });
});
