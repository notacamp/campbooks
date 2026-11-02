import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MARTA } from "../../../../../test/demo/attention";
import {
  CLIENTS_ID,
  OPTIONS,
  P,
  PROJECT_ROW,
  PROJECTS_ID,
  ROW,
  TablesServer,
  withoutTrashed,
} from "../../../../../test/demo/tables";
import { ANA, DUARTE, RUI } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { tablesApi } from "../../../../../test/tables";
import type { PropertyView, RowView } from "../types";

import { CellDisplay } from "./CellDisplay";
import type { CellContext } from "./context";

const server = new TablesServer();
const detail = () => server.table(CLIENTS_ID);

function property(id: string): PropertyView {
  const found = detail()?.properties.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`no property ${id}`);
  }
  return found as PropertyView;
}

const row = (id: string): RowView => {
  const found = server.rowOf(id);
  if (found === undefined) {
    throw new Error(`no row ${id}`);
  }
  return withoutTrashed(found);
};

const ctx: CellContext = {
  workspaceId: DUARTE.id,
  locale: "en",
  members: [ANA, RUI],
  open: vi.fn(),
};

function show(
  propertyOrType: PropertyView | Partial<PropertyView>,
  value?: unknown,
  rowId = ROW.acme,
) {
  tablesApi(server);
  const base = row(rowId);
  const resolved =
    "id" in propertyOrType && "name" in propertyOrType && "position" in propertyOrType
      ? (propertyOrType as PropertyView)
      : ({
          id: "extra",
          name: "Extra",
          position: "a9",
          config: {},
          can_revert: false,
          ...propertyOrType,
        } as PropertyView);
  return renderWithProviders(
    <CellDisplay ctx={ctx} property={resolved} row={base} value={value} />,
  );
}

describe("what a cell shows", () => {
  it("shows a title, a number in its format, a select as a tag and a person by name", async () => {
    const title = show(property(P.name), "ignored");
    expect(await screen.findByText("Acme Studio")).toBeTruthy();
    title.unmount();
    const number = show(property(P.value), "9600");
    // A narrow column cuts the number with an ellipsis; the whole of it is in the title.
    expect(await screen.findByText("€9,600.00")).toHaveAttribute("title", "€9,600.00");
    number.unmount();
    const select = show(property(P.stage), OPTIONS.kickoff);
    expect(await screen.findByText("Kickoff")).toBeTruthy();
    select.unmount();
    show(property(P.owner), [ANA.user_id, "0192f1a0-0000-7000-8000-0000000000ff"]);
    expect(await screen.findByText("Ana Duarte")).toBeTruthy();
    expect(screen.getByText("Someone")).toBeTruthy();
  });

  it("shows the options of a multi-select, a date range and a text with a safe link only", async () => {
    const multi = show(
      {
        type: "multi_select",
        config: {
          options: [
            { id: "a", name: "Dental", color: "blue" },
            { id: "b", name: "Bakery", color: "red" },
          ],
        },
      },
      ["a", "b", "gone"],
    );
    expect(await screen.findByText("Dental")).toBeTruthy();
    expect(screen.getByText("Bakery")).toBeTruthy();
    multi.unmount();
    const date = show(property(P.next), { start: "2026-10-30", end: "2026-11-02" });
    expect((await screen.findByText(/→/u)).textContent).toContain("30");
    date.unmount();
    show({ type: "text" }, [
      { text: "See " },
      { text: "this", href: "https://example.com/a" },
      { text: " and ", href: "javascript:alert(1)" },
      { text: "that", href: "mailto:ana@example.com" },
    ]);
    expect(await screen.findByRole("link", { name: "this" })).toHaveAttribute(
      "href",
      "https://example.com/a",
    );
    expect(screen.getByRole("link", { name: "that" })).toHaveAttribute(
      "href",
      "mailto:ana@example.com",
    );
    // The script link is text, not a link.
    expect(screen.queryByRole("link", { name: /and/u })).toBeNull();
    expect(screen.getByText(/and/u)).toBeTruthy();
  });

  it("links a url and an email only when they are safe, and shows a phone as text", async () => {
    const url = show({ type: "url" }, "https://example.com/page");
    expect(await screen.findByRole("link", { name: "https://example.com/page" })).toHaveAttribute(
      "target",
      "_blank",
    );
    url.unmount();
    const unsafe = show({ type: "url" }, "javascript:alert(1)");
    expect(await screen.findByText("javascript:alert(1)")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    unsafe.unmount();
    const email = show({ type: "email" }, "ana@example.com");
    expect(await screen.findByRole("link", { name: "ana@example.com" })).toHaveAttribute(
      "href",
      "mailto:ana@example.com",
    );
    email.unmount();
    const phone = show({ type: "phone" }, "+351 912 345 678");
    expect(await screen.findByText("+351 912 345 678")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    phone.unmount();
    show({ type: "phone" }, null);
    expect(screen.queryByText(/\d/u)).toBeNull();
  });

  it("shows files as downloads of the row's page", async () => {
    show({ type: "files" }, [
      { hash: "a".repeat(64), name: "Contract.pdf", size: 10, type: "application/pdf" },
    ]);
    const link = await screen.findByRole("link", { name: "Contract.pdf" });
    expect(link.getAttribute("href")).toContain(`/pages/${ROW.acme}/files/${"a".repeat(64)}`);
  });

  it("shows the links to Campbooks records by their own names, and a record it cannot open as that", async () => {
    show(property(P.threads), [
      { type: "conversations.conversation", id: MARTA.id },
      { type: "tasks.task", id: "0192f1a0-0000-7000-8000-00000000dead" },
      { type: "documents.document", id: "x" },
    ]);
    expect(await screen.findByText("Walkthrough on Thursday")).toBeTruthy();
    await waitFor(() => {
      expect(screen.getAllByText("Something you can’t open")).toHaveLength(2);
    });
  });

  it("shows the rows a relation names by their titles", async () => {
    show(property(P.projects), [PROJECT_ROW, "0192f1a0-0000-7000-8000-00000000beef"]);
    expect(await screen.findByText("Acme rebrand")).toBeTruthy();
    expect(await screen.findByText("Something you can’t open")).toBeTruthy();
    expect(PROJECTS_ID).toBeTruthy();
  });

  it("shows when and by whom a row was made or edited, and its number", async () => {
    const made = show({ type: "created", config: { of: "time" } });
    expect(await screen.findByText(/2026/u)).toBeTruthy();
    made.unmount();
    const who = show({ type: "edited", config: { of: "who" } });
    expect(await screen.findByText("Ana Duarte")).toBeTruthy();
    who.unmount();
    const id = show({ type: "unique_id", config: { prefix: "CL" } });
    expect(await screen.findByText("CL-1")).toBeTruthy();
    id.unmount();
    const bare = show({ type: "unique_id", config: { prefix: "" } });
    expect(await screen.findByText("1")).toBeTruthy();
    bare.unmount();
  });

  it("shows a checkbox and what a property kept in Notion holds", async () => {
    const box = show({ type: "checkbox", name: "Signed" }, true);
    expect(await screen.findByRole("checkbox", { name: "Signed" })).toBeChecked();
    box.unmount();
    const formula = show({ type: "notion_formula" }, "42");
    expect(await screen.findByText("42")).toBeTruthy();
    formula.unmount();
    show({ type: "external" }, null);
    expect(await screen.findByText("Kept in Notion")).toBeTruthy();
    expect(within(document.body).queryByRole("link")).toBeNull();
  });
});
