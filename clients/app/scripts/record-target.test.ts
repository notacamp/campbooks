import { describe, expect, it } from "vitest";

import { refuseTarget } from "./record-target.ts";

describe("refuseTarget", () => {
  it("allows this machine", () => {
    for (const url of [
      "http://localhost:5173",
      "http://127.0.0.1:3000/",
      "https://localhost",
      "http://[::1]:5173",
    ]) {
      expect(refuseTarget(url, false)).toBeUndefined();
    }
  });

  it("refuses any other host, production included", () => {
    for (const url of [
      "https://app.example.com",
      "https://staging.example.com",
      "http://192.168.1.20:5173",
      "http://localhost.example.com",
      "http://127.0.0.1.nip.io",
    ]) {
      expect(refuseTarget(url, false)).toMatch(/^refusing to record/u);
    }
  });

  it("records elsewhere only with the explicit flag", () => {
    expect(refuseTarget("https://staging.example.com", true)).toBeUndefined();
  });

  it("refuses what is not an http(s) URL, flag or not", () => {
    expect(refuseTarget("localhost:5173", true)).toMatch(/not an http/u);
    expect(refuseTarget("file:///tmp/x", true)).toMatch(/not an http/u);
    expect(refuseTarget("not a url", true)).toMatch(/not a URL/u);
  });
});
