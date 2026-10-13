import { describe, expect, it } from "vitest";

import { ApiError, errorStatus, problemCode, unwrapEmpty } from "./errors";

describe("problemCode", () => {
  it("reads the stable code of an API error, not its localized detail", () => {
    const error = new ApiError(409, {
      title: "Conflit",
      status: 409,
      detail: "Un espace de travail a besoin d’au moins un propriétaire.",
      code: "last_owner",
    });

    expect(problemCode(error)).toBe("last_owner");
    expect(errorStatus(error)).toBe(409);
  });

  it("is undefined for errors without a code", () => {
    expect(problemCode(new ApiError(404, { title: "Not Found", status: 404 }))).toBeUndefined();
    expect(problemCode(new ApiError(500, null))).toBeUndefined();
    expect(problemCode(new ApiError(400, { code: 3 }))).toBeUndefined();
    expect(problemCode(new ApiError(400, { detail: "last_owner" }))).toBeUndefined();
    expect(problemCode(new Error("boom"))).toBeUndefined();
    expect(errorStatus(new Error("boom"))).toBeUndefined();
  });
});

describe("unwrapEmpty", () => {
  it("accepts an empty success and throws on a failure", () => {
    expect(() => {
      unwrapEmpty({ response: new Response(null, { status: 204 }) });
    }).not.toThrow();
    expect(() => {
      unwrapEmpty({ response: new Response(null, { status: 409 }), error: { detail: "x" } });
    }).toThrow(ApiError);
  });
});
