import { describe, expect, it } from "vitest";

import { mockApi } from "../../../../test/render";

import {
  RUNNABLE,
  runnable,
  runRequest,
  undoBody,
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

/** A row a later module adds (an event deleted, undone with its token): not one of ours. */
const FAKE: Readonly<Record<string, Runnable>> = {
  "schedule.delete_event": { method: "DELETE", path: "events/{event_id}", records: [] },
  "schedule.restore_event": { method: "POST", path: "events/{event_id}/restore", records: [] },
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
      ].toSorted(),
    );
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
      expect(runnable(odd), JSON.stringify(odd)).toBeUndefined();
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
    operation: "schedule.restore_event",
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
      operation: "schedule.delete_event",
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
