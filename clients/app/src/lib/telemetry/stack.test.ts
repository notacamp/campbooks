import { describe, expect, it } from "vitest";

import { MAX_FRAMES, parseStack } from "./stack";

describe("parseStack", () => {
  it("reads V8 frames, named and anonymous", () => {
    const stack = [
      "TypeError: Cannot read properties of undefined",
      "    at renderThread (https://app.example.com/assets/index-3f2c1e0.js:12:34)",
      "    at https://app.example.com/assets/vendor-9a8b.js:1:2",
    ].join("\n");
    expect(parseStack(stack)).toEqual([
      {
        file: "https://app.example.com/assets/index-3f2c1e0.js",
        function: "renderThread",
        line: 12,
        column: 34,
      },
      { file: "https://app.example.com/assets/vendor-9a8b.js", function: null, line: 1, column: 2 },
    ]);
  });

  it("reads WebKit and Gecko frames", () => {
    expect(parseStack("renderThread@https://app.example.com/assets/index.js:7:9")).toEqual([
      {
        file: "https://app.example.com/assets/index.js",
        function: "renderThread",
        line: 7,
        column: 9,
      },
    ]);
  });

  it("keeps at most MAX_FRAMES frames and nothing without a stack", () => {
    const stack = Array.from({ length: 50 }, () => "    at f (https://a.example/x.js:1:1)").join(
      "\n",
    );
    expect(parseStack(stack)).toHaveLength(MAX_FRAMES);
    expect(parseStack()).toEqual([]);
  });
});
