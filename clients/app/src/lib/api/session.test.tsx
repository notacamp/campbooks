import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { ApiError, useSession } from "./index";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

describe("useSession", () => {
  it("reads the signed-in user from the BFF", async () => {
    const session = {
      authenticated: true,
      user: { sub: "user-1", name: "Ada Lovelace", email: "ada@example.com" },
      expires_at: "2026-11-12T10:00:00Z",
    };
    const fetchSpy = vi.fn((request: Request) => {
      expect(new URL(request.url).pathname).toBe("/bff/session");
      expect(request.credentials).toBe("same-origin");
      return Promise.resolve(Response.json(session));
    });
    vi.stubGlobal("fetch", fetchSpy);

    const { result } = renderHook(() => useSession(), { wrapper });

    await waitFor(() => {
      expect(result.current.data).toStrictEqual(session);
    });
    expect(fetchSpy).toHaveBeenCalledOnce();
  });

  it("reports a signed-out browser as data, not an error", async () => {
    vi.stubGlobal("fetch", () => Promise.resolve(Response.json({ authenticated: false })));

    const { result } = renderHook(() => useSession(), { wrapper });

    await waitFor(() => {
      expect(result.current.data).toStrictEqual({ authenticated: false });
    });
  });

  it("surfaces a failing BFF as an ApiError", async () => {
    vi.stubGlobal("fetch", () =>
      Promise.resolve(Response.json({ title: "Service Unavailable" }, { status: 503 })),
    );

    const { result } = renderHook(() => useSession(), { wrapper });

    await waitFor(() => {
      expect(result.current.error).toBeInstanceOf(ApiError);
    });
  });
});
