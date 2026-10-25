import { describe, expect, it } from "vitest";

import { codeLanguage } from "./languages";

describe("codeLanguage", () => {
  it("keeps the languages the server accepts and maps the usual short names", () => {
    expect(codeLanguage("rust")).toBe("rust");
    expect(codeLanguage("TS")).toBe("typescript");
    expect(codeLanguage(" py ")).toBe("python");
    expect(codeLanguage("c++")).toBe("cpp");
  });

  it("drops what it does not know", () => {
    expect(codeLanguage("brainfuck")).toBeNull();
    expect(codeLanguage("")).toBeNull();
    expect(codeLanguage(undefined)).toBeNull();
  });
});
