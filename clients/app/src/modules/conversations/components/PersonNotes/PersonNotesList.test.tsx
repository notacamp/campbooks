import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { PersonNotesList } from "./PersonNotesList";

const P = `${W}/person-notes`;
const MARTA = {
  id: "0192f1a0-0000-7000-8000-0000000002b1",
  address: "marta@clientes.example",
  name: "Marta Ferreira",
  note: "My landlord. Formal, in Portuguese.",
  updated_at: "2026-10-12T09:00:00Z",
};
const RUI = {
  id: "0192f1a0-0000-7000-8000-0000000002b2",
  address: "rui@obras.example",
  note: "My builder. Tu, short messages.",
  updated_at: "2026-10-11T09:00:00Z",
};

type Note = typeof MARTA | typeof RUI;

/** The member's notes, kept as the server would. */
function notesApi(start: Note[]) {
  let notes: Note[] = [...start];
  // What was ever kept, so Undo brings a note back as it was.
  const known = [...start];
  return mockApi({
    [`GET ${P}`]: () => ({ body: { notes } }),
    [`PUT ${P}`]: (_request, body) => {
      const { address, note } = body as { address: string; note: string };
      const before = notes.find((item) => item.address === address) ??
        known.find((item) => item.address === address) ?? { ...RUI, address };
      const kept = { ...before, note };
      notes = [kept, ...notes.filter((item) => item.address !== address)];
      return { body: kept };
    },
    [`DELETE ${P}/${MARTA.id}`]: () => {
      notes = notes.filter((item) => item.id !== MARTA.id);
      return { status: 204 };
    },
  });
}

function list() {
  renderWithProviders(<PersonNotesList workspaceId={DUARTE.id} />);
}

describe("PersonNotesList", () => {
  it("lists each person by name, or address, with what Scout knows", async () => {
    notesApi([MARTA, RUI]);
    list();
    const notes = await screen.findByRole("list", { name: "People you told Scout about" });
    const [first, second, ...more] = within(notes).getAllByRole("listitem");
    if (first === undefined || second === undefined) {
      throw new Error("two rows expected");
    }
    expect(more).toHaveLength(0);
    expect(within(first).getByText("Marta Ferreira")).toBeVisible();
    expect(within(first).getByText("marta@clientes.example")).toBeVisible();
    expect(within(first).getByText(MARTA.note)).toBeVisible();
    expect(within(second).getByText("rui@obras.example")).toBeVisible();
  });

  it("says what to do when there is no one yet", async () => {
    notesApi([]);
    list();
    expect(
      await screen.findByText(
        "When you reply to someone, you can tell Scout who they are and how to write to them.",
      ),
    ).toBeVisible();
  });

  it("says it could not load them, with Try again", async () => {
    let fail = true;
    mockApi({
      [`GET ${P}`]: () => (fail ? problem(503, "unavailable") : { body: { notes: [MARTA] } }),
    });
    list();
    expect(
      await screen.findByText("Could not load what you told Scout about people"),
    ).toBeVisible();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText(MARTA.note)).toBeVisible();
  });

  it("edits a note in place, then puts the focus back on Edit", async () => {
    const api = notesApi([MARTA]);
    list();
    fireEvent.click(
      await screen.findByRole("button", { name: "Edit what Scout knows about Marta Ferreira" }),
    );
    const field = screen.getByRole("textbox", {
      name: "About Marta Ferreira, and how to write to them",
    });
    expect(field).toHaveValue(MARTA.note);
    fireEvent.change(field, { target: { value: "My landlord. Você, never tu." } });
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(await screen.findByText("My landlord. Você, never tu.")).toBeVisible();
    expect(api.calls.find((c) => c.route === `PUT ${P}`)?.body).toEqual({
      address: MARTA.address,
      note: "My landlord. Você, never tu.",
    });
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Edit what Scout knows about Marta Ferreira" }),
      ).toHaveFocus();
    });
    expect(screen.getAllByRole("status").some((s) => s.textContent === "Saved")).toBe(true);
  });

  it("cancels an edit with Esc, keeping the note as it was", async () => {
    const api = notesApi([MARTA]);
    list();
    fireEvent.click(
      await screen.findByRole("button", { name: "Edit what Scout knows about Marta Ferreira" }),
    );
    const field = screen.getByRole("textbox", { name: /About Marta Ferreira/u });
    fireEvent.change(field, { target: { value: "Something else" } });
    fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByText(MARTA.note)).toBeVisible();
    expect(api.calls.some((c) => c.route === `PUT ${P}`)).toBe(false);
  });

  it("removes a note with Undo, which keeps it again", async () => {
    const api = notesApi([MARTA, RUI]);
    list();
    fireEvent.click(
      await screen.findByRole("button", { name: "Remove what Scout knows about Marta Ferreira" }),
    );
    await waitFor(() => {
      expect(screen.queryByText(MARTA.note)).toBeNull();
    });
    expect(api.calls.map((c) => c.route)).toContain(`DELETE ${P}/${MARTA.id}`);
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    expect(await screen.findByText(MARTA.note)).toBeVisible();
    expect(api.calls.findLast((c) => c.route === `PUT ${P}`)?.body).toEqual({
      address: MARTA.address,
      note: MARTA.note,
    });
  });
});
