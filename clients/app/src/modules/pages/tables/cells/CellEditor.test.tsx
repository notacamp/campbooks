import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  CLIENTS_ID,
  OPTIONS,
  P,
  ROW,
  TablesServer,
  withoutTrashed,
} from "../../../../../test/demo/tables";
import { ANA, DUARTE, RUI } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { tablesApi } from "../../../../../test/tables";
import type { PropertyView, RowView } from "../types";

import { CellEditor } from "./CellEditor";
import type { CellContext } from "./context";
import { must } from "../../../../../test/must";

const server = new TablesServer();

const property = (over: Partial<PropertyView>): PropertyView => ({
  id: "extra",
  name: "Extra",
  type: "text",
  config: {},
  position: "a9",
  can_revert: false,
  ...over,
});

const row = (): RowView => {
  return withoutTrashed(must(server.rowOf(ROW.acme)));
};

const ctx: CellContext = {
  workspaceId: DUARTE.id,
  locale: "en",
  members: [ANA, RUI],
  open: vi.fn(),
};

function edit(
  prop: PropertyView,
  value: unknown,
  seed?: string,
  over: Partial<Parameters<typeof CellEditor>[0]> = {},
) {
  tablesApi(new TablesServer());
  const onCommit = vi.fn();
  const onCancel = vi.fn();
  const onOptions = vi.fn(() => Promise.resolve(undefined));
  const upload = vi.fn();
  renderWithProviders(
    <CellEditor
      ctx={ctx}
      property={prop}
      row={row()}
      value={value}
      seed={seed}
      pages={[]}
      canEditOptions
      onCommit={onCommit}
      onCancel={onCancel}
      onOptions={onOptions}
      upload={upload}
      {...over}
    />,
  );
  return { onCommit, onCancel, onOptions, upload };
}

const field = () => screen.getByRole("textbox");

describe("a text field in a cell", () => {
  it("commits what was typed on Enter, on Tab and on leaving the field", () => {
    for (const leave of ["enter", "tab", "blur"] as const) {
      const { onCommit } = edit(property({ type: "text" }), [{ text: "Old" }]);
      expect(field()).toHaveValue("Old");
      fireEvent.change(field(), { target: { value: "New words" } });
      if (leave === "blur") {
        fireEvent.blur(field());
      } else {
        fireEvent.keyDown(field(), { key: leave === "enter" ? "Enter" : "Tab" });
      }
      expect(onCommit).toHaveBeenCalledWith([{ text: "New words" }]);
      document.body.innerHTML = "";
    }
  });

  it("changes nothing when nothing changed, or when Escape is pressed", () => {
    const first = edit(property({ type: "text" }), [{ text: "Same" }]);
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(first.onCommit).not.toHaveBeenCalled();
    expect(first.onCancel).toHaveBeenCalledTimes(1);
    document.body.innerHTML = "";
    const second = edit(property({ type: "text" }), [{ text: "Same" }]);
    fireEvent.change(field(), { target: { value: "Other" } });
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(second.onCommit).not.toHaveBeenCalled();
    expect(second.onCancel).toHaveBeenCalledTimes(1);
  });

  it("clears the cell when the text is emptied", () => {
    const { onCommit } = edit(property({ type: "text" }), [{ text: "Gone" }]);
    fireEvent.change(field(), { target: { value: "  " } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith(null);
  });

  it("starts from the character typed to begin the edit", () => {
    edit(property({ type: "text" }), [{ text: "Old" }], "Z");
    expect(field()).toHaveValue("Z");
  });

  it("takes an http or https address for a url, and refuses anything else", () => {
    const good = edit(property({ type: "url" }), null);
    fireEvent.change(field(), { target: { value: "https://example.com/x" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(good.onCommit).toHaveBeenCalledWith("https://example.com/x");
    document.body.innerHTML = "";
    for (const bad of ["javascript:alert(1)", "mailto:a@b.co", "example.com", "ftp://x.example"]) {
      const refused = edit(property({ type: "url" }), null);
      fireEvent.change(field(), { target: { value: bad } });
      expect(field()).toHaveAttribute("aria-invalid", "true");
      fireEvent.keyDown(field(), { key: "Enter" });
      expect(refused.onCommit).not.toHaveBeenCalled();
      // Leaving the field with something that is no address leaves the cell as it was.
      fireEvent.blur(field());
      expect(refused.onCancel).toHaveBeenCalled();
      document.body.innerHTML = "";
    }
  });

  it("takes an email address, a phone number and a number", () => {
    const email = edit(property({ type: "email" }), null);
    fireEvent.change(field(), { target: { value: "ana@example.com" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(email.onCommit).toHaveBeenCalledWith("ana@example.com");
    document.body.innerHTML = "";
    const phone = edit(property({ type: "phone" }), "+351 912");
    fireEvent.change(field(), { target: { value: "" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(phone.onCommit).toHaveBeenCalledWith(null);
    document.body.innerHTML = "";
    const number = edit(property({ type: "number" }), "5");
    fireEvent.change(field(), { target: { value: "7,5" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(number.onCommit).toHaveBeenCalledWith("7.5");
    document.body.innerHTML = "";
    const cleared = edit(property({ type: "number" }), "5");
    fireEvent.change(field(), { target: { value: "" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(cleared.onCommit).toHaveBeenCalledWith(null);
    document.body.innerHTML = "";
    const bad = edit(property({ type: "email" }), null);
    fireEvent.change(field(), { target: { value: "not an address" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(bad.onCommit).not.toHaveBeenCalled();
  });

  it("writes the title as it was typed, trimmed", () => {
    const { onCommit } = edit(property({ type: "title" }), "Acme Studio");
    fireEvent.change(field(), { target: { value: "  Acme  " } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith("Acme");
  });
});

describe("a picker in a cell", () => {
  const status = property({
    id: P.stage,
    type: "status",
    config: {
      options: [
        { id: "a", name: "To do", color: "gray", group: "todo" },
        { id: "b", name: "Done", color: "green", group: "done" },
      ],
    },
  });

  it("commits a single choice at once", async () => {
    const { onCommit } = edit(status, "a");
    fireEvent.click(await screen.findByRole("option", { name: "Done" }));
    expect(onCommit).toHaveBeenCalledWith("b");
  });

  it("clears the cell from the Clear button", async () => {
    const { onCommit } = edit(status, "a");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    expect(onCommit).toHaveBeenCalledWith(null);
  });

  it("writes several choices when it closes, but nothing if nothing changed", async () => {
    const multi = property({
      type: "multi_select",
      config: {
        options: [
          { id: "a", name: "Dental", color: "blue" },
          { id: "b", name: "Bakery", color: "red" },
        ],
      },
    });
    const changed = edit(multi, ["a"]);
    const search = await screen.findByRole("combobox", { name: "Search options" });
    fireEvent.click(screen.getByRole("option", { name: "Bakery" }));
    fireEvent.keyDown(search, { key: "Escape" });
    await waitFor(() => {
      expect(changed.onCommit).toHaveBeenCalledWith(["a", "b"]);
    });
    document.body.innerHTML = "";
    const same = edit(multi, ["a"]);
    fireEvent.keyDown(await screen.findByRole("combobox", { name: "Search options" }), {
      key: "Escape",
    });
    await waitFor(() => {
      expect(same.onCancel).toHaveBeenCalled();
    });
    expect(same.onCommit).not.toHaveBeenCalled();
  });

  it("renames, recolours and deletes an option from its edit button", async () => {
    const { onOptions } = edit(status, "a");
    fireEvent.click(await screen.findByRole("button", { name: "Edit option Done" }));
    fireEvent.change(screen.getByLabelText("Option name"), { target: { value: "Finished" } });
    fireEvent.change(screen.getByLabelText("Colour"), { target: { value: "purple" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(onOptions).toHaveBeenCalledWith(
        [
          { id: "a", name: "To do", color: "gray", group: "todo" },
          { id: "b", name: "Finished", color: "purple", group: "done" },
        ],
        expect.any(Array),
      );
    });
    fireEvent.click(await screen.findByRole("button", { name: "Edit option Done" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete option" }));
    await waitFor(() => {
      expect(onOptions).toHaveBeenLastCalledWith(
        [{ id: "a", name: "To do", color: "gray", group: "todo" }],
        expect.any(Array),
      );
    });
  });

  it("goes back from editing an option without changing it", async () => {
    const { onOptions } = edit(status, "a");
    fireEvent.click(await screen.findByRole("button", { name: "Edit option Done" }));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("combobox", { name: "Search options" })).toBeTruthy();
    expect(onOptions).not.toHaveBeenCalled();
  });

  it("makes a new status option in the to-do group, picks it, and cannot when options are fixed", async () => {
    const sent: unknown[][] = [];
    const made = vi.fn((next: readonly unknown[]) => {
      sent.push([...next]);
      return Promise.resolve({
        ...status,
        config: {
          options: [
            ...(status.config as { options: unknown[] }).options,
            { id: "new", name: "Blocked", color: "gray", group: "todo" },
          ],
        },
      });
    });
    const { onCommit } = edit(status, "a", undefined, { onOptions: made });
    fireEvent.change(await screen.findByRole("combobox", { name: "Search options" }), {
      target: { value: "Blocked" },
    });
    fireEvent.click(screen.getByRole("option", { name: "Create “Blocked”" }));
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith("new");
    });
    expect(sent[0]).toContainEqual(expect.objectContaining({ name: "Blocked", group: "todo" }));
    document.body.innerHTML = "";
    edit(status, "a", undefined, { canEditOptions: false });
    fireEvent.change(await screen.findByRole("combobox", { name: "Search options" }), {
      target: { value: "Whatever" },
    });
    expect(screen.queryByRole("option", { name: /Create/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Edit option/u })).toBeNull();
  });

  it("walks the options with the arrow keys and picks with Enter", async () => {
    const { onCommit } = edit(status, "a");
    const search = await screen.findByRole("combobox", { name: "Search options" });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith("b");
    document.body.innerHTML = "";
    const again = edit(status, "a");
    const box = await screen.findByRole("combobox", { name: "Search options" });
    fireEvent.keyDown(box, { key: "ArrowUp" });
    fireEvent.keyDown(box, { key: "Enter" });
    expect(again.onCommit).toHaveBeenCalledWith("b");
  });

  it("writes the people picked when it closes", async () => {
    const { onCommit } = edit(property({ type: "person" }), [ANA.user_id]);
    const search = await screen.findByRole("combobox", { name: "Search people" });
    fireEvent.click(screen.getByRole("option", { name: "Rui Matos" }));
    fireEvent.keyDown(search, { key: "Escape" });
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith([ANA.user_id, RUI.user_id]);
    });
  });

  it("writes a day picked from the calendar when it closes", async () => {
    const date = property({ id: P.next, type: "date", config: { time: false } });
    const { onCommit } = edit(date, { start: "2026-10-30" });
    fireEvent.click(
      await screen.findByRole("button", { name: /October 31, 2026|31 October 2026/u }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith({ start: "2026-10-31" });
    });
  });

  it("cancels a date edit nobody touched", async () => {
    const date = property({ type: "date", config: {} });
    const { onCancel } = edit(date, { start: "2026-10-30" });
    fireEvent.click(await screen.findByRole("button", { name: "Done" }));
    await waitFor(() => {
      expect(onCancel).toHaveBeenCalled();
    });
  });

  it("uploads a file for the row and writes the list when it closes", async () => {
    const upload = vi.fn(() =>
      Promise.resolve({ blob: "d".repeat(64), size: 3, content_type: "text/plain" }),
    );
    const { onCommit } = edit(property({ type: "files" }), null, undefined, { upload });
    const input = await screen.findByLabelText("Choose a file");
    fireEvent.change(input, {
      target: { files: [new File(["abc"], "Notes.txt", { type: "text/plain" })] },
    });
    expect(await screen.findByText("Notes.txt")).toBeTruthy();
    expect(upload).toHaveBeenCalledWith(ROW.acme, expect.any(File));
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith([
        { hash: "d".repeat(64), name: "Notes.txt", size: 3, type: "text/plain" },
      ]);
    });
  });

  it("has no editor for a column that is filled in for you", () => {
    edit(property({ type: "created" }), null);
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(within(document.body).queryByRole("dialog")).toBeNull();
    expect(OPTIONS.lead).toBeTruthy();
    expect(CLIENTS_ID).toBeTruthy();
  });
});
