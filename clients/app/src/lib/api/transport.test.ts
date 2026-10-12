import { describe, expect, it, vi } from "vitest";

import { TransportError } from "./errors";
import {
  createTauriTransport,
  createWebTransport,
  selectTransport,
  type Invoke,
} from "./transport";

const ORIGIN = "http://localhost:3000";

describe("createWebTransport", () => {
  it("sends same-origin requests to the BFF with the session cookie only", async () => {
    const send = vi.fn<(request: Request) => Promise<Response>>(() =>
      Promise.resolve(new Response("{}")),
    );
    const transport = createWebTransport(ORIGIN, send);

    await transport.fetch(
      new Request(`${transport.baseUrl}/api/v1/system/health`, {
        headers: { Authorization: "Bearer leaked", Accept: "application/json" },
      }),
    );

    expect(transport.baseUrl).toBe(`${ORIGIN}/bff`);
    const sent = send.mock.calls[0]?.[0];
    expect(sent?.url).toBe(`${ORIGIN}/bff/api/v1/system/health`);
    expect(sent?.credentials).toBe("same-origin");
    expect(sent?.mode).toBe("same-origin");
    expect(sent?.headers.get("authorization")).toBeNull();
    expect(sent?.headers.get("accept")).toBe("application/json");
  });

  it("forwards request bodies", async () => {
    const send = vi.fn((request: Request) => request.text().then((text) => new Response(text)));
    const transport = createWebTransport(ORIGIN, send);

    const response = await transport.fetch(
      new Request(`${transport.baseUrl}/api/v1/things`, { method: "POST", body: '{"a":1}' }),
    );

    expect(await response.text()).toBe('{"a":1}');
  });

  it.each([
    ["another origin", "https://evil.example.com/bff/api/v1/system/health"],
    ["a path outside the BFF", `${ORIGIN}/api/v1/system/health`],
  ])("refuses %s", async (_, url) => {
    const send = vi.fn(() => Promise.resolve(new Response()));
    const transport = createWebTransport(ORIGIN, send);

    await expect(transport.fetch(new Request(url))).rejects.toBeInstanceOf(TransportError);
    expect(send).not.toHaveBeenCalled();
  });

  it("defaults to the page origin and global fetch", async () => {
    const fetchSpy = vi.fn(() => Promise.resolve(new Response("{}")));
    vi.stubGlobal("fetch", fetchSpy);
    const transport = createWebTransport();

    await transport.fetch(new Request(`${transport.baseUrl}/api/v1/system/health`));

    expect(transport.baseUrl).toBe(`${globalThis.location.origin}/bff`);
    expect(fetchSpy).toHaveBeenCalledOnce();
  });
});

describe("createTauriTransport", () => {
  it("hands method, path, query and JSON body to the api_request command", async () => {
    const invoke = vi.fn<Invoke>(() => Promise.resolve({ status: 201, body: { id: "1" } }));
    const transport = createTauriTransport(invoke);

    const response = await transport.fetch(
      new Request(`${transport.baseUrl}/api/v1/things?tag=a&tag=b`, {
        method: "POST",
        body: JSON.stringify({ name: "x" }),
      }),
    );

    expect(invoke).toHaveBeenCalledWith("api_request", {
      method: "POST",
      path: "/api/v1/things",
      query: [
        ["tag", "a"],
        ["tag", "b"],
      ],
      body: { name: "x" },
    });
    expect(response.status).toBe(201);
    expect(response.headers.get("content-type")).toBe("application/json");
    expect(await response.json()).toStrictEqual({ id: "1" });
  });

  it("maps an empty response body to an empty Response", async () => {
    const transport = createTauriTransport(() => Promise.resolve({ status: 204, body: null }));

    const response = await transport.fetch(
      new Request(`${transport.baseUrl}/api/v1/things/1`, { method: "DELETE" }),
    );

    expect(response.status).toBe(204);
    expect(await response.text()).toBe("");
  });

  it("sends a null body for body-less requests", async () => {
    const invoke = vi.fn<Invoke>(() => Promise.resolve({ status: 200, body: {} }));
    const transport = createTauriTransport(invoke);

    await transport.fetch(new Request(`${transport.baseUrl}/api/v1/system/health`));

    expect(invoke.mock.calls[0]?.[1].body).toBeNull();
  });

  it("rejects non-JSON bodies", async () => {
    const invoke = vi.fn<Invoke>();
    const transport = createTauriTransport(invoke);

    await expect(
      transport.fetch(
        new Request(`${transport.baseUrl}/api/v1/things`, { method: "POST", body: "plain" }),
      ),
    ).rejects.toBeInstanceOf(TransportError);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("wraps bridge failures in a TransportError", async () => {
    const transport = createTauriTransport(() => Promise.reject(new Error("path not allowed")));

    await expect(
      transport.fetch(new Request(`${transport.baseUrl}/api/v1/system/health`)),
    ).rejects.toThrow("Native API bridge failed: Error: path not allowed");
  });
});

describe("selectTransport", () => {
  it("uses the web transport in a browser", () => {
    expect(selectTransport().kind).toBe("web");
  });

  it("uses the native bridge inside Tauri", () => {
    vi.stubGlobal("isTauri", true);

    expect(selectTransport().kind).toBe("tauri");
  });
});
