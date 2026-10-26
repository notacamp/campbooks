import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CLIENTS_ID, OPTIONS, P, TablesServer } from "../../../../../test/demo/tables";
import { ANA, DUARTE, RUI } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { tablesApi } from "../../../../../test/tables";
import type { CellContext } from "../cells/context";
import type { FilterNode, PropertyView } from "../types";

import { FilterBuilder, SortBuilder } from "./Builders";

const server = new TablesServer();
const properties = (server.table(CLIENTS_ID)?.properties ?? []) as PropertyView[];

const ctx: CellContext = {
  workspaceId: DUARTE.id,
  locale: "en",
  members: [ANA, RUI],
  open: vi.fn(),
};

function render(filter?: FilterNode, focus?: string) {
  tablesApi(new TablesServer());
  const onApply = vi.fn();
  const onClose = vi.fn();
  renderWithProviders(
    <FilterBuilder
      ctx={ctx}
      properties={[...properties, ...extra]}
      filter={filter}
      pages={[]}
      focus={focus}
      onApply={onApply}
      onClose={onClose}
    />,
  );
  return { onApply, onClose };
}

// Properties of types the demo table does not have.
const extra: PropertyView[] = [
  {
    id: "text-1",
    name: "Notes",
    type: "text",
    config: {},
    position: "b0",
    can_revert: false,
  },
  {
    id: "created-1",
    name: "Created by",
    type: "created",
    config: { of: "who" },
    position: "b1",
    can_revert: false,
  },
  {
    id: "created-2",
    name: "Created",
    type: "created",
    config: { of: "time" },
    position: "b2",
    can_revert: false,
  },
  {
    id: "notion",
    name: "Formula",
    type: "notion_formula",
    config: {},
    position: "b3",
    can_revert: false,
  },
];

const condition = (n: number) => screen.getByRole("group", { name: `Condition ${String(n)}` });

describe("the filter builder", () => {
  it("starts empty, adds a condition on the first property and applies it", () => {
    const { onApply, onClose } = render();
    expect(screen.getByText("No conditions yet.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add condition" }));
    const first = condition(1);
    fireEvent.change(within(first).getByLabelText("Value"), { target: { value: "acme" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onApply).toHaveBeenCalledWith({
      and: [{ property: P.name, operator: "is", value: "acme" }],
    });
    expect(onClose).toHaveBeenCalled();
  });

  it("leaves out a condition that has no value yet", () => {
    const { onApply } = render();
    fireEvent.click(screen.getByRole("button", { name: "Add condition" }));
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onApply).toHaveBeenCalledWith(undefined);
  });

  it("keeps the properties it cannot filter on out of the list", () => {
    render();
    fireEvent.click(screen.getByRole("button", { name: "Add condition" }));
    const names = within(condition(1))
      .getAllByRole("option")
      .map((option) => option.textContent);
    expect(names).toContain("Notes");
    expect(names).not.toContain("Formula");
    expect(names).not.toContain("Projects".replace("Projects", "Threads-gone"));
  });

  it("starts on the column it was opened from", () => {
    render(undefined, P.value);
    expect(within(condition(1)).getByLabelText("Property")).toHaveValue(P.value);
  });

  it("changes a condition's property and operator, starting each afresh", () => {
    render({ and: [{ property: P.name, operator: "contains", value: "x" }] });
    const first = condition(1);
    fireEvent.change(within(first).getByLabelText("Property"), { target: { value: P.stage } });
    expect(within(first).getByLabelText("Condition")).toHaveValue("is");
    expect(within(first).getByLabelText("Value")).toHaveValue(OPTIONS.lead);
    fireEvent.change(within(first).getByLabelText("Condition"), { target: { value: "is_any_of" } });
    fireEvent.click(within(first).getByRole("checkbox", { name: "Kickoff" }));
    fireEvent.click(within(first).getByRole("checkbox", { name: "Active" }));
    fireEvent.change(within(first).getByLabelText("Condition"), {
      target: { value: "is_empty" },
    });
    expect(within(first).queryByLabelText("Value")).toBeNull();
  });

  it("reads each kind of value", () => {
    const { onApply } = render({
      and: [
        { property: P.signed, operator: "is", value: true },
        { property: P.next, operator: "after", value: "2026-10-30" },
        { property: P.owner, operator: "contains", value: RUI.user_id },
        { property: P.value, operator: "gt", value: "5000" },
        { property: "created-1", operator: "is", value: ANA.user_id },
      ],
    });
    // A checkbox is yes or no.
    fireEvent.change(within(condition(1)).getByLabelText("Value"), { target: { value: "no" } });
    // A date is a calendar, a person a list of teammates by name.
    expect(within(condition(2)).getByRole("button", { name: /Value/u })).toBeTruthy();
    expect(within(condition(3)).getByLabelText("Value")).toHaveValue(RUI.user_id);
    expect(within(condition(4)).getByLabelText("Value")).toHaveValue("5000");
    expect(within(condition(5)).getByLabelText("Value")).toHaveValue(ANA.user_id);
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    const sent = onApply.mock.calls[0]?.[0] as { and: { value: unknown }[] };
    expect(sent.and[0]?.value).toBe(false);
  });

  it("matches all or any when there is more than one condition", () => {
    const { onApply } = render({
      and: [
        { property: P.name, operator: "contains", value: "a" },
        { property: P.name, operator: "contains", value: "b" },
      ],
    });
    fireEvent.click(screen.getByRole("button", { name: "Any" }));
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onApply.mock.calls[0]?.[0]).toHaveProperty("or");
  });

  it("removes a condition, and clears them all", () => {
    const { onApply } = render({
      and: [
        { property: P.name, operator: "contains", value: "a" },
        { property: P.name, operator: "contains", value: "b" },
      ],
    });
    fireEvent.click(screen.getByRole("button", { name: "Remove condition 1" }));
    expect(screen.queryByRole("group", { name: "Condition 2" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Clear filter" }));
    expect(onApply).toHaveBeenCalledWith();
  });

  it("shows a filter with groups as it is, and only lets it be cleared", () => {
    const { onApply } = render({
      and: [
        {
          or: [
            { property: P.name, operator: "contains", value: "a" },
            { property: P.name, operator: "contains", value: "b" },
          ],
        },
        { property: P.name, operator: "contains", value: "c" },
      ],
    });
    expect(screen.getByText(/groups of conditions/u)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add condition" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onApply).toHaveBeenCalledWith({
      and: expect.arrayContaining([expect.anything()]) as unknown,
    });
  });
});

describe("the sort builder", () => {
  function renderSort(sorts = [] as { property: string; direction: "asc" | "desc" }[]) {
    const onApply = vi.fn();
    const onClose = vi.fn();
    renderWithProviders(
      <SortBuilder properties={properties} sorts={sorts} onApply={onApply} onClose={onClose} />,
    );
    return { onApply, onClose };
  }

  it("adds sorts on properties that can be sorted, one each, and applies them", () => {
    const { onApply, onClose } = renderSort();
    expect(screen.getByText("Not sorted.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add sort" }));
    fireEvent.click(screen.getByRole("button", { name: "Add sort" }));
    const second = screen.getByRole("group", { name: "Sort 2" });
    fireEvent.change(within(second).getByLabelText("Direction"), { target: { value: "desc" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    const sent = onApply.mock.calls[0]?.[0] as { property: string; direction: string }[];
    expect(sent).toHaveLength(2);
    expect(sent[0]?.property).not.toBe(sent[1]?.property);
    expect(sent[1]?.direction).toBe("desc");
    expect(onClose).toHaveBeenCalled();
  });

  it("changes the property of a sort and removes one", () => {
    const { onApply } = renderSort([
      { property: P.value, direction: "asc" },
      { property: P.name, direction: "asc" },
    ]);
    fireEvent.change(
      within(screen.getByRole("group", { name: "Sort 1" })).getByLabelText("Property"),
      {
        target: { value: P.stage },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove sort 2" }));
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onApply).toHaveBeenCalledWith([{ property: P.stage, direction: "asc" }]);
  });

  it("stops at five", () => {
    renderSort();
    for (let i = 0; i < 6; i += 1) {
      const add = screen.queryByRole("button", { name: "Add sort" });
      if (add !== null) {
        fireEvent.click(add);
      }
    }
    expect(screen.getAllByRole("group", { name: /^Sort \d/u })).toHaveLength(5);
    expect(screen.queryByRole("button", { name: "Add sort" })).toBeNull();
  });
});
