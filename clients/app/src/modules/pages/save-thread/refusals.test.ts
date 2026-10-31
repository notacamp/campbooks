import { describe, expect, it } from "vitest";

import { ApiError } from "~/lib/api";

import { FILE_MAX_BYTES, refusalOf, tooLarge } from "./refusals";
import { refusalText } from "./wording";

const refused = (status: number, code?: string) =>
  new ApiError(status, code === undefined ? undefined : { code });

describe("refusalOf", () => {
  const files = [{ name: "brand-guide.pdf", size: 1024 }];

  it("names a sign-in that may not read mail (403 insufficient_scope)", () => {
    expect(refusalOf(refused(403, "insufficient_scope"), files)).toEqual({ reason: "scope" });
  });

  it("names a thread that changed or a file that is gone (404)", () => {
    expect(refusalOf(refused(404), files)).toEqual({ reason: "changed" });
  });

  it("names the file over 20 MiB (413)", () => {
    expect(refusalOf(refused(413, "file_too_large"), files)).toEqual({
      reason: "tooLarge",
      name: "brand-guide.pdf",
    });
    const big = [
      { name: "logo-pack.zip", size: FILE_MAX_BYTES + 1 },
      { name: "notes.txt", size: 10 },
    ];
    expect(refusalOf(refused(413), big)).toEqual({ reason: "tooLarge", name: "logo-pack.zip" });
  });

  it("says the file is too large without a name when it cannot tell which", () => {
    expect(
      refusalOf(refused(413), [
        { name: "a.pdf", size: 5 },
        { name: "b.pdf", size: 5 },
      ]),
    ).toEqual({
      reason: "tooLarge",
      name: undefined,
    });
  });

  it("is a plain failure for anything else", () => {
    expect(refusalOf(refused(500), files)).toEqual({ reason: "failed" });
    expect(refusalOf(new Error("offline"), files)).toEqual({ reason: "failed" });
  });
});

describe("tooLarge", () => {
  it("finds the first file over the cap, and none at the cap", () => {
    expect(tooLarge([{ name: "a", size: FILE_MAX_BYTES }])).toBeUndefined();
    expect(
      tooLarge([
        { name: "a", size: 1 },
        { name: "b", size: FILE_MAX_BYTES + 1 },
      ])?.name,
    ).toBe("b");
  });
});

describe("refusalText", () => {
  it("says each refusal in its own words, with the file's name", () => {
    const t = (key: string, options: { name?: string } = {}) =>
      `${key}${options.name === undefined ? "" : `:${options.name}`}`;
    expect(refusalText(t, { reason: "scope" })).toBe("saveToPage.refused.scope");
    expect(refusalText(t, { reason: "changed" })).toBe("saveToPage.refused.changed");
    expect(refusalText(t, { reason: "tooLarge", name: "logo-pack.zip" })).toBe(
      "saveToPage.refused.tooLarge:logo-pack.zip",
    );
    expect(refusalText(t, { reason: "tooLarge" })).toBe("saveToPage.refused.tooLargeSome");
  });
});
