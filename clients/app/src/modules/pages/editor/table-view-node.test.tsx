import { EditorContent, useEditor } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { W } from "../../../../test/demo/conversations";
import {
  CLIENTS_ID,
  PROJECTS_ID,
  ROW,
  TablesServer,
  VIEW_BOARD,
} from "../../../../test/demo/tables";
import { DUARTE } from "../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../test/render";
import { tablesApi } from "../../../../test/tables";

import { Bid } from "./bid";
import { EditorEnvironmentContext } from "./environment";
import { TableViewBlock } from "./nodes";
import { must } from "../../../../test/must";

vi.mock("./PageEditor", () => ({ PageEditor: () => <p>the row’s page</p> }));

const open = vi.fn();

function Editing({ table, view }: { table: string; view: string }) {
  const editor = useEditor({
    extensions: [StarterKit, Bid, TableViewBlock],
    editable: true,
    content: {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Our clients:" }] },
        { type: "tableView", attrs: { table, view } },
      ],
    },
  });
  return (
    <EditorEnvironmentContext value={{ workspaceId: DUARTE.id, pageId: "page-1", open }}>
      <EditorContent editor={editor} />
    </EditorEnvironmentContext>
  );
}

describe("a table in a page", () => {
  it("shows the table's own view, read for the viewer, with a link to the table", async () => {
    tablesApi();
    renderWithProviders(<Editing table={CLIENTS_ID} view={VIEW_BOARD} />);
    expect(await screen.findByText("Our clients:")).toBeTruthy();
    // The block names the view it was made for.
    expect(await screen.findByRole("tab", { name: "By stage", selected: true })).toBeTruthy();
    expect(await screen.findByRole("list", { name: "Kickoff" })).toBeTruthy();
    const link = screen.getByRole("link", { name: "Clients" });
    expect(link).toHaveAttribute("href", `/w/${DUARTE.id}/pages/${CLIENTS_ID}`);
    fireEvent.click(link);
    expect(open).toHaveBeenCalledWith(`/w/${DUARTE.id}/pages/${CLIENTS_ID}`);
  });

  it("keeps its keys to itself: arrows in the table do not move the editor's caret", async () => {
    tablesApi();
    renderWithProviders(<Editing table={CLIENTS_ID} view="" />);
    const grid = await screen.findByRole("grid");
    const cell = must(within(grid).getAllByRole("gridcell")[0]);
    const event = new KeyboardEvent("keydown", {
      key: "ArrowDown",
      bubbles: true,
      cancelable: true,
    });
    cell.dispatchEvent(event);
    expect(screen.getByText("Our clients:")).toBeTruthy();
    expect(ROW.acme).toBeTruthy();
  });

  it("says only that something is there when the viewer may not open the table", async () => {
    tablesApi(new TablesServer(), { [`GET ${W}/tables/${PROJECTS_ID}`]: () => ({ status: 404 }) });
    renderWithProviders(<Editing table={PROJECTS_ID} view="x" />);
    expect(await screen.findByText("Something you can’t open")).toBeTruthy();
    await waitFor(() => {
      expect(screen.queryByRole("grid")).toBeNull();
    });
  });

  it("says the same for a block that names no table", async () => {
    tablesApi();
    renderWithProviders(<Editing table="" view="" />);
    expect(await screen.findByText("Something you can’t open")).toBeTruthy();
  });
});
