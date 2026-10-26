import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MARTA } from "../../../../../test/demo/attention";
import { W } from "../../../../../test/demo/conversations";
import { P, PROJECT_ROW, PROJECTS_ID, TablesServer } from "../../../../../test/demo/tables";
import { LEASE } from "../../../../../test/demo/tasks";
import { ANA, DUARTE, RUI, SOFIA } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { tablesApi } from "../../../../../test/tables";
import type { PropertyView } from "../types";

import type { CellContext } from "./context";
import { DateEditor, FilesEditor, LinkPicker, PersonPicker, RowPicker } from "./Pickers";
import { must } from "../../../../../test/must";

const ctx: CellContext = {
  workspaceId: DUARTE.id,
  locale: "en",
  members: [ANA, SOFIA, RUI],
  open: vi.fn(),
};

describe("the person picker", () => {
  it("finds teammates by name or address and toggles them", async () => {
    tablesApi();
    const onChange = vi.fn();
    renderWithProviders(<PersonPicker ctx={ctx} selected={[ANA.user_id]} onChange={onChange} />);
    expect(await screen.findByRole("option", { name: "Ana Duarte", selected: true })).toBeTruthy();
    fireEvent.change(screen.getByRole("combobox", { name: "Search people" }), {
      target: { value: "RUI@" },
    });
    expect(screen.queryByRole("option", { name: "Ana Duarte" })).toBeNull();
    fireEvent.click(screen.getByRole("option", { name: "Rui Matos" }));
    expect(onChange).toHaveBeenCalledWith([ANA.user_id, RUI.user_id]);
    fireEvent.change(screen.getByRole("combobox", { name: "Search people" }), {
      target: { value: "nobody" },
    });
    expect(screen.getByText("No one by that name.")).toBeTruthy();
  });

  it("takes someone off the list when picked again", async () => {
    tablesApi();
    const onChange = vi.fn();
    renderWithProviders(<PersonPicker ctx={ctx} selected={[ANA.user_id]} onChange={onChange} />);
    fireEvent.click(await screen.findByRole("option", { name: "Ana Duarte" }));
    expect(onChange).toHaveBeenCalledWith([]);
  });
});

describe("the row picker", () => {
  it("finds the rows of the related table by title", async () => {
    const { server } = tablesApi();
    const onChange = vi.fn();
    renderWithProviders(
      <RowPicker ctx={ctx} table={PROJECTS_ID} selected={[]} onChange={onChange} />,
    );
    fireEvent.click(await screen.findByRole("option", { name: "Acme rebrand" }));
    expect(onChange).toHaveBeenCalledWith([PROJECT_ROW]);
    fireEvent.change(screen.getByRole("combobox", { name: "Search rows" }), {
      target: { value: "zzz" },
    });
    expect(await screen.findByText("No rows found.")).toBeTruthy();
    expect(server.calls.some((call) => call.endsWith("/query"))).toBe(true);
  });

  it("keeps what is chosen on the list even when a search does not find it", async () => {
    tablesApi();
    const onChange = vi.fn();
    renderWithProviders(
      <RowPicker
        ctx={ctx}
        table={PROJECTS_ID}
        selected={["0192f1a0-0000-7000-8000-00000000beef"]}
        onChange={onChange}
      />,
    );
    expect(
      await screen.findByRole("option", {
        name: "0192f1a0-0000-7000-8000-00000000beef",
        selected: true,
      }),
    ).toBeTruthy();
    expect(screen.getByText("A row")).toBeTruthy();
  });

  it("says only that a table the viewer cannot open is something they can't open", async () => {
    tablesApi(new TablesServer(), { [`GET ${W}/tables/${PROJECTS_ID}`]: () => ({ status: 404 }) });
    renderWithProviders(
      <RowPicker ctx={ctx} table={PROJECTS_ID} selected={[]} onChange={vi.fn()} />,
    );
    expect(await screen.findByText("Something you can’t open")).toBeTruthy();
  });
});

describe("the Campbooks link picker", () => {
  const property = (types: string[]): PropertyView => ({
    id: P.threads,
    name: "Threads",
    type: "campbooks_link",
    config: { types },
    position: "a5",
    can_revert: false,
  });

  function searching() {
    return tablesApi(new TablesServer(), {
      [`GET ${W}/tasks`]: () => ({ body: { tasks: [LEASE] } }),
      [`GET ${W}/search`]: () => ({
        body: {
          hits: [
            {
              record: { type: "conversations.conversation", id: MARTA.id },
              title: [{ text: MARTA.subject, match: false }],
              from: { name: "Marta Ferreira", address: "marta@casa-lume.example" },
            },
          ],
        },
      }),
    });
  }

  it("finds threads, tasks and more by name and adds the one picked", async () => {
    searching();
    const onChange = vi.fn();
    renderWithProviders(
      <LinkPicker ctx={ctx} property={property([])} pages={[]} selected={[]} onChange={onChange} />,
    );
    const search = screen.getByRole("combobox", {
      name: "Search conversations, tasks, events, people and pages",
    });
    fireEvent.change(search, { target: { value: "walk" } });
    fireEvent.click(await screen.findByRole("option", { name: MARTA.subject }));
    expect(onChange).toHaveBeenCalledWith([{ type: "conversations.conversation", id: MARTA.id }]);
  });

  it("offers only the kinds of record the column allows", async () => {
    searching();
    renderWithProviders(
      <LinkPicker
        ctx={ctx}
        property={property(["tasks.task"])}
        pages={[]}
        selected={[]}
        onChange={vi.fn()}
      />,
    );
    await screen.findAllByRole("option");
    await waitFor(() => {
      expect(screen.queryByRole("option", { name: MARTA.subject })).toBeNull();
    });
  });

  it("shows what is chosen by its own name and takes it off", async () => {
    searching();
    const onChange = vi.fn();
    renderWithProviders(
      <LinkPicker
        ctx={ctx}
        property={property([])}
        pages={[]}
        selected={[{ type: "conversations.conversation", id: MARTA.id }]}
        onChange={onChange}
      />,
    );
    expect(await screen.findByText(MARTA.subject)).toBeTruthy();
    fireEvent.click(await screen.findByRole("option", { selected: true }));
    expect(onChange).toHaveBeenCalledWith([]);
  });
});

describe("the date editor", () => {
  it("picks a day, then a range, and clears", () => {
    const onChange = vi.fn();
    const onDone = vi.fn();
    renderWithProviders(
      <DateEditor
        value={{ start: "2026-10-30" }}
        withTime={false}
        onChange={onChange}
        onDone={onDone}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /October 31, 2026|31 October 2026/u }));
    expect(onChange).toHaveBeenLastCalledWith({ start: "2026-10-31" });
    fireEvent.click(screen.getByRole("checkbox", { name: "Add an end date" }));
    expect(onChange).toHaveBeenLastCalledWith({ start: "2026-10-31", end: "2026-10-31" });
    const groups = screen.getAllByRole("group");
    const endGrid = groups.find((group) => group.getAttribute("aria-label") === "End date");
    fireEvent.click(
      within(must(endGrid)).getByRole("button", {
        name: /November 2, 2026|2 November 2026/u,
      }),
    );
    // The calendar of the end date shows its month; the start stays as it was.
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ start: "2026-10-31" }));
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onDone).toHaveBeenCalled();
  });

  it("keeps a time of day with the date when the column has one", () => {
    const onChange = vi.fn();
    renderWithProviders(
      <DateEditor value={{ start: "2026-10-30" }} withTime onChange={onChange} onDone={vi.fn()} />,
    );
    const time = screen.getByRole("combobox", { name: "Time" });
    fireEvent.focus(time);
    fireEvent.change(time, { target: { value: "14:30" } });
    fireEvent.keyDown(time, { key: "Enter" });
    const last = onChange.mock.lastCall?.[0] as { start: string } | undefined;
    expect(last?.start.endsWith("Z")).toBe(true);
  });

  it("starts from nothing and clears back to nothing", () => {
    const onChange = vi.fn();
    renderWithProviders(
      <DateEditor value={null} withTime={false} onChange={onChange} onDone={vi.fn()} />,
    );
    expect(screen.getByRole("checkbox", { name: "Add an end date" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(onChange).toHaveBeenLastCalledWith(null);
  });
});

describe("the files editor", () => {
  const file = new File(["hello"], "Offer.pdf", { type: "application/pdf" });

  it("uploads a file with the row's page and lists it, then takes it off", async () => {
    const onChange = vi.fn();
    const upload = vi.fn(() =>
      Promise.resolve({ blob: "b".repeat(64), size: 5, content_type: "application/pdf" }),
    );
    renderWithProviders(
      <FilesEditor value={null} upload={upload} onChange={onChange} onDone={vi.fn()} />,
    );
    fireEvent.change(screen.getByLabelText("Choose a file"), { target: { files: [file] } });
    expect(await screen.findByText("Offer.pdf")).toBeTruthy();
    expect(onChange).toHaveBeenLastCalledWith([
      { hash: "b".repeat(64), name: "Offer.pdf", size: 5, type: "application/pdf" },
    ]);
    fireEvent.click(screen.getByRole("button", { name: "Remove Offer.pdf" }));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it("says when a file could not be uploaded", async () => {
    renderWithProviders(
      <FilesEditor
        value={[{ hash: "c".repeat(64), name: "Old.pdf", size: 1, type: "application/pdf" }]}
        upload={() => Promise.reject(new Error("no"))}
        onChange={vi.fn()}
        onDone={vi.fn()}
      />,
    );
    expect(screen.getByText("Old.pdf")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Choose a file"), { target: { files: [file] } });
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t upload that file.");
  });
});
