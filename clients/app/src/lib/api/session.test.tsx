import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { readPref, writePref } from "~/lib/storage/prefs";

import { ApiError, signOut, signOutAndLeave, useSession } from "./index";

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

describe("signOut", () => {
  it("posts to the BFF with the CSRF header and returns the end-session URL", async () => {
    const fetchSpy = vi.fn((request: Request) => {
      expect(new URL(request.url).pathname).toBe("/bff/logout");
      expect(request.method).toBe("POST");
      expect(request.headers.get("x-campbooks-csrf")).toBe("1");
      return Promise.resolve(Response.json({ logout_url: "https://id.example.com/logout" }));
    });
    vi.stubGlobal("fetch", fetchSpy);

    await expect(signOut()).resolves.toBe("https://id.example.com/logout");
  });

  it("returns null when the identity provider has no end-session URL", async () => {
    vi.stubGlobal("fetch", () => Promise.resolve(Response.json({ logout_url: null })));

    await expect(signOut()).resolves.toBeNull();
  });
});

describe("signOutAndLeave", () => {
  it("goes to the end-session URL, or to the root without one", async () => {
    const leave = vi.fn();
    vi.stubGlobal("fetch", () =>
      Promise.resolve(Response.json({ logout_url: "https://id.example.com/logout" })),
    );
    await signOutAndLeave(leave);
    vi.stubGlobal("fetch", () => Promise.resolve(Response.json({ logout_url: null })));
    await signOutAndLeave(leave);

    expect(leave.mock.calls).toEqual([["https://id.example.com/logout"], ["/"]]);
  });
});

describe("recent searches on a shared browser", () => {
  it("are forgotten on sign-out, for every account and workspace", async () => {
    writePref("searches.user-1.w1", '["from:client invoice"]');
    writePref("searches.user-2.w2", '["lease"]');
    writePref("locale", "fr");
    vi.stubGlobal("fetch", () => Promise.resolve(Response.json({ logout_url: null })));

    await signOut();

    expect(readPref("searches.user-1.w1")).toBeNull();
    expect(readPref("searches.user-2.w2")).toBeNull();
    // Only the queries go: preferences stay.
    expect(readPref("locale")).toBe("fr");
    writePref("locale", "en");
  });

  it("are forgotten when the session is found to be over", async () => {
    writePref("searches.user-1.w1", '["lease"]');
    vi.stubGlobal("fetch", () => Promise.resolve(Response.json({ authenticated: false })));

    const { result } = renderHook(() => useSession(), { wrapper });

    await waitFor(() => {
      expect(result.current.data).toStrictEqual({ authenticated: false });
    });
    expect(readPref("searches.user-1.w1")).toBeNull();
  });
});
