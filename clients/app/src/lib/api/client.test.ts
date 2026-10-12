import { describe, expect, it } from "vitest";

import { ApiError, createApiClient, unwrap, type Transport } from "./index";

function transportReturning(status: number, body: unknown): Transport {
  return {
    kind: "web",
    baseUrl: "http://localhost/bff",
    fetch: () => Promise.resolve(Response.json(body, { status })),
  };
}

describe("createApiClient", () => {
  it("returns typed data for a successful call", async () => {
    const client = createApiClient(transportReturning(200, { status: "ok", database: "ok" }));

    const health = unwrap(await client.GET("/api/v1/system/health"));

    expect(health).toStrictEqual({ status: "ok", database: "ok" });
  });

  it("throws an ApiError carrying the problem document", async () => {
    const problem = { type: "about:blank", title: "Not Found", status: 404 };
    const client = createApiClient(transportReturning(404, problem));

    const result = await client.GET("/api/v1/system/version");

    expect(() => unwrap(result)).toThrow(ApiError);
    expect(() => unwrap(result)).toThrow(
      expect.objectContaining({ status: 404, problem }) as ApiError,
    );
  });
});
