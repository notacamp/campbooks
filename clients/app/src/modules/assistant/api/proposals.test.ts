import { describe, expect, it } from "vitest";

import { mockApi } from "../../../../test/render";

import {
  PAGE_UNDOS,
  RUNNABLE,
  runnable,
  runRequest,
  undoBody,
  withAnswer,
  type ProposalRequest,
  type Runnable,
} from "./proposals";

const W = "/bff/api/v1/w/ws-1";
const C = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";

const archive: ProposalRequest = {
  operation: "conversations.archive",
  method: "POST",
  path: `conversations/${C}/archive`,
  body: null,
};

/** A row a later module adds (a note deleted, undone with its token): not one of ours. */
const FAKE: Readonly<Record<string, Runnable>> = {
  "notes.delete": { method: "DELETE", path: "events/{event_id}", records: [] },
  "notes.restore": { method: "POST", path: "events/{event_id}/restore", records: [] },
};

describe("the client's own actions", () => {
  it("are the server's action table and its undos", () => {
    expect(Object.keys(RUNNABLE).toSorted()).toEqual(
      [
        "attention.snooze",
        "attention.unsnooze",
        "conversations.add_label",
        "conversations.archive",
        "conversations.confirm_need",
        "conversations.mark_read",
        "conversations.mark_unread",
        "conversations.move_to_stream",
        "conversations.not_spam",
        "conversations.remove_label",
        "conversations.reopen_need",
        "conversations.report_spam",
        "conversations.resolve_need",
        "conversations.star",
        "conversations.trash",
        "conversations.unarchive",
        "conversations.unstar",
        "conversations.untrash",
        "pages.append",
        "pages.create",
        "schedule.create_event",
        "schedule.delete_event",
        "schedule.push_event",
        "schedule.respond_event",
        "schedule.restore_event",
        "schedule.unpush_event",
        "schedule.update_event",
        "tasks.done",
        "tasks.hand_off",
        "tasks.not_needed",
        "tasks.plan",
        "tasks.reopen",
        "tasks.take_back",
        "tasks.update",
      ].toSorted(),
    );
  });

  it("gain exactly the two page changes; their undos are the page card's own", () => {
    expect(Object.keys(RUNNABLE).filter((name) => name.startsWith("pages."))).toEqual([
      "pages.append",
      "pages.create",
    ]);
    expect(Object.keys(PAGE_UNDOS)).toEqual(["pages.unappend", "pages.trash"]);
    // A page's undo is not something any other card may send.
    const unappend: ProposalRequest = {
      operation: "pages.unappend",
      method: "POST",
      path: `pages/${C}/unappend`,
      body: null,
    };
    expect(runnable(unappend)).toBeUndefined();
    expect(runnable(unappend, PAGE_UNDOS)?.params).toEqual({ page_id: C });
  });

  it("run only their own operation, method and path, under the workspace", () => {
    expect(runnable(archive)?.params).toEqual({ conversation_id: C });
    expect(
      runnable({
        ...archive,
        operation: "conversations.remove_label",
        method: "DELETE",
        path: `conversations/${C}/labels/l-1`,
      })?.params,
    ).toEqual({ conversation_id: C, label_id: "l-1" });
    for (const odd of [
      { ...archive, operation: "conversations.send_draft" },
      { ...archive, operation: "toString" },
      { ...archive, method: "DELETE" as const },
      { ...archive, method: "GET" as unknown as "POST" },
      { ...archive, path: `conversations/${C}/unarchive` },
      { ...archive, path: `../../workspaces/ws-2/conversations/${C}/archive` },
      { ...archive, path: `conversations/../archive` },
      { ...archive, path: `conversations/${C}%2F..%2Fx/archive` },
      { ...archive, path: `/api/v1/w/ws-2/conversations/${C}/archive` },
    ]) {
      expect(runnable(odd)).toBeUndefined();
    }
  });

  it("send the member's own request with the proposal as its key", async () => {
    const seen: { key: string | null; url: string }[] = [];
    mockApi({
      [`POST ${W}/conversations/${C}/archive`]: (request) => {
        seen.push({ key: request.headers.get("Idempotency-Key"), url: request.url });
        return { body: { id: C, archived: true } };
      },
    });

    const answer = await runRequest("ws-1", archive, "p-1", null);

    expect(answer).toMatchObject({ archived: true });
    expect(seen).toEqual([
      { key: "p-1", url: expect.stringContaining(`/w/ws-1/conversations/${C}/archive`) as string },
    ]);
  });
});

describe("an undo that takes fields of the change's answer", () => {
  const undo: ProposalRequest = {
    operation: "notes.restore",
    method: "POST",
    path: "events/e-1/restore",
    body: { note: "kept" },
    from_response: ["undo_token"],
  };

  it("copies exactly the named top-level fields, and nothing else", () => {
    expect(
      undoBody(undo, { undo_token: "t-9", title: "Walkthrough", nested: { undo_token: "no" } }),
    ).toEqual({ note: "kept", undo_token: "t-9" });
    expect(undoBody(undo, "not an object")).toEqual({ note: "kept" });
    expect(undoBody({ ...archive, body: null }, { undo_token: "t-9" })).toBeNull();
  });

  it("is sent with the copied fields, keyed `<id>:undo`, for a row the list holds", async () => {
    const sent: { key: string | null; body: unknown }[] = [];
    mockApi({
      [`DELETE ${W}/events/e-1`]: { body: { undo_token: "t-9", id: "e-1" } },
      [`POST ${W}/events/e-1/restore`]: (request, body) => {
        sent.push({ key: request.headers.get("Idempotency-Key"), body });
        return { body: { id: "e-1" } };
      },
    });
    const change: ProposalRequest = {
      operation: "notes.delete",
      method: "DELETE",
      path: "events/e-1",
      body: null,
      from_response: [],
    };

    const answer = await runRequest("ws-1", change, "p-2", null, FAKE);
    await runRequest("ws-1", undo, "p-2:undo", undoBody(undo, answer), FAKE);

    expect(sent).toEqual([{ key: "p-2:undo", body: { note: "kept", undo_token: "t-9" } }]);
    // Not one of the app's own actions.
    expect(runnable(change)).toBeUndefined();
  });
});

describe("a task's cards", () => {
  const T = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";
  const edit: ProposalRequest = {
    operation: "tasks.update",
    method: "PATCH",
    path: `tasks/${T}`,
    body: { revision: 7, title: "Renew the van insurance", verb: "do", due_on: "2026-10-30" },
  };

  it("run their exact paths, PATCH for the edit alone", () => {
    expect(runnable(edit)?.params).toEqual({ task_id: T });
    expect(
      runnable({ ...edit, operation: "tasks.done", method: "POST", path: `tasks/${T}/done` })
        ?.params,
    ).toEqual({ task_id: T });
    for (const odd of [
      { ...edit, method: "POST" as const },
      { ...edit, path: `tasks/${T}/plan` },
      { ...edit, path: `tasks/../x` },
      // PATCH is not allowed for any other operation, even with a matching entry.
      { ...edit, operation: "tasks.plan", path: `tasks/${T}/plan` },
      { ...archive, method: "PATCH" as const },
      { ...edit, operation: "tasks.create", path: "tasks" },
    ]) {
      expect(runnable(odd)).toBeUndefined();
    }
    const patchOnly: Readonly<Record<string, Runnable>> = {
      "notes.edit": { method: "PATCH", path: "events/{event_id}", records: [] },
    };
    expect(
      runnable(
        { operation: "notes.edit", method: "PATCH", path: "events/e-1", body: null },
        patchOnly,
      ),
    ).toBeUndefined();
  });

  it("send the edit as a PATCH with the proposal as its key", async () => {
    const seen: { key: string | null; body: unknown }[] = [];
    mockApi({
      [`PATCH ${W}/tasks/${T}`]: (request, body) => {
        seen.push({ key: request.headers.get("Idempotency-Key"), body });
        return { body: { id: T, revision: 8 } };
      },
    });
    await runRequest("ws-1", edit, "p-3", edit.body);
    expect(seen).toEqual([{ key: "p-3", body: edit.body }]);
  });

  it("undo with the new revision and nothing else of the answer", () => {
    const undo: ProposalRequest = {
      operation: "tasks.update",
      method: "PATCH",
      path: `tasks/${T}`,
      body: { title: "Renew the van insurance", verb: "do", due_on: null, due_at: null },
      from_response: ["revision"],
    };
    const answer = {
      id: T,
      revision: 8,
      title: "A title another change wrote",
      due_on: "2026-10-30",
      assignee: "u-1",
    };
    expect(undoBody(undo, answer)).toEqual({
      title: "Renew the van insurance",
      verb: "do",
      due_on: null,
      due_at: null,
      revision: 8,
    });
  });
});

describe("an event's cards", () => {
  const E = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2c0001";
  const edit: ProposalRequest = {
    operation: "schedule.update_event",
    method: "PATCH",
    path: `schedule/events/${E}`,
    body: { place: "Clinic" },
  };
  /** The undo of a new event: it deletes the event the answer names. */
  const deleteNew: ProposalRequest = {
    operation: "schedule.delete_event",
    method: "DELETE",
    path: "schedule/events/{event_id}",
    body: null,
    from_response: ["@event_id:id"],
  };
  const putBack: ProposalRequest = {
    operation: "schedule.push_event",
    method: "POST",
    path: "schedule/events/{event_id}/push",
    body: null,
    from_response: ["@event_id:id", "calendar_id"],
  };

  it("run their exact paths, PATCH for the edit", () => {
    expect(runnable(edit)?.params).toEqual({ event_id: E });
    expect(
      runnable({
        operation: "schedule.create_event",
        method: "POST",
        path: "schedule/events",
        body: { title: "Dentist", starts: "2026-11-20T10:00:00+00:00" },
      })?.params,
    ).toEqual({});
    expect(
      runnable({
        operation: "schedule.restore_event",
        method: "POST",
        path: `schedule/events/${E}/restore`,
        body: null,
        from_response: ["undo_token"],
      })?.params,
    ).toEqual({ event_id: E });
    for (const odd of [
      { ...edit, method: "POST" as const },
      { ...edit, path: `schedule/events/${E}/push` },
      { ...edit, path: "schedule/events/../x" },
      // PATCH is for a task's edit and an event's alone.
      { ...edit, operation: "schedule.respond_event", path: `schedule/events/${E}/response` },
      // A placeholder is only for a path the answer fills in.
      { ...edit, path: "schedule/events/{event_id}" },
      { ...deleteNew, from_response: [] },
      { ...deleteNew, from_response: ["@other_id:id"] },
    ]) {
      expect(runnable(odd)).toBeUndefined();
    }
  });

  it("wait for the id the change's answer gives, and run nowhere else", () => {
    expect(runnable(deleteNew)?.entry.method).toBe("DELETE");
    expect(withAnswer(deleteNew, { id: E, title: "Dentist" })?.path).toBe(`schedule/events/${E}`);
    expect(withAnswer(putBack, { id: E, calendar_id: "c-1" })?.path).toBe(
      `schedule/events/${E}/push`,
    );
    // The body takes only the named fields; the `@` entry is the path's.
    expect(undoBody(putBack, { id: E, calendar_id: "c-1", title: "x" })).toEqual({
      calendar_id: "c-1",
    });
    expect(undoBody(deleteNew, { id: E })).toBeNull();
    for (const answer of [
      undefined,
      "not an object",
      {},
      { id: 7 },
      { id: "../../x" },
      { id: "a/b" },
      { id: `${E}%2F..` },
    ]) {
      expect(withAnswer(deleteNew, answer)).toBeUndefined();
    }
    // A request with nothing to wait for is as it was.
    expect(withAnswer(edit, null)).toBe(edit);
  });

  it("send a new event's undo to the event the answer names", async () => {
    const sent: { key: string | null }[] = [];
    mockApi({
      [`POST ${W}/schedule/events`]: { body: { id: E } },
      [`DELETE ${W}/schedule/events/${E}`]: (request) => {
        sent.push({ key: request.headers.get("Idempotency-Key") });
        return { body: { id: E, undo_token: "t-1" } };
      },
    });
    const create: ProposalRequest = {
      operation: "schedule.create_event",
      method: "POST",
      path: "schedule/events",
      body: { title: "Dentist", starts: "2026-11-20T10:00:00+00:00" },
    };

    const answer = await runRequest("ws-1", create, "p-4", create.body);
    const target = withAnswer(deleteNew, answer);
    if (target === undefined) {
      throw new Error("the answer names the event the undo deletes");
    }
    await runRequest("ws-1", target, "p-4:undo", undoBody(deleteNew, answer));

    expect(sent).toEqual([{ key: "p-4:undo" }]);
  });
});
