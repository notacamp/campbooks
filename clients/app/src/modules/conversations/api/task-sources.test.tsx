import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { LEASE_ASK, LEASE_MESSAGE, LEASE_QUOTE } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { MARTA } from "../../../../test/demo/attention";
import { mockApi, problem } from "../../../../test/render";

import { useAskQuote } from "./task-sources";

const READ = `GET ${W}/conversations/${MARTA.id}/messages/${LEASE_MESSAGE}/read`;

const read = {
  status: "done",
  layers: ["model"],
  items: [],
  asks: [
    {
      key: LEASE_ASK,
      title: "Send the signed lease",
      quote: LEASE_QUOTE,
      verb: "do",
      done_by: "elsewhere",
    },
  ],
  flagged: false,
  can_read: false,
};

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

describe("useAskQuote", () => {
  it("is the sender's sentence for a member who may read the email", async () => {
    mockApi({ [READ]: { body: read } });
    const { result } = renderHook(() => useAskQuote(DUARTE.id, MARTA.id, LEASE_ASK), { wrapper });
    await waitFor(() => {
      expect(result.current).toBe(LEASE_QUOTE);
    });
  });

  it("is nothing for a member the email is hidden from (the server answers 404)", async () => {
    const api = mockApi({ [READ]: problem(404, "not_found") });
    const { result } = renderHook(() => useAskQuote(DUARTE.id, MARTA.id, LEASE_ASK), { wrapper });
    await waitFor(() => {
      expect(api.calls).toHaveLength(1);
    });
    expect(result.current).toBeUndefined();
  });

  it("is nothing, and asks nothing, for a task made without an ask or a conversation", () => {
    const api = mockApi({});
    const none = renderHook(() => useAskQuote(DUARTE.id, MARTA.id, null), { wrapper });
    const other = renderHook(() => useAskQuote(DUARTE.id, undefined, LEASE_ASK), { wrapper });
    expect(none.result.current).toBeUndefined();
    expect(other.result.current).toBeUndefined();
    expect(api.calls).toHaveLength(0);
  });
});
