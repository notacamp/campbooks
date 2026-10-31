// @vitest-environment node
import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import {
  BLOCKED_EXTENSIONS,
  extensionOf,
  isBlocked,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENTS,
  megabytes,
  refusalOfCode,
  screen,
} from "./attachment-rules";

const rust = readFileSync(
  new URL(
    "../../../../../../crates/modules/conversations/src/domain/attachments.rs",
    import.meta.url,
  ),
  "utf8",
);
const drafts = readFileSync(
  new URL("../../../../../../crates/modules/conversations/src/domain/drafts.rs", import.meta.url),
  "utf8",
);

const file = (name: string, size = 10) => new File([new Uint8Array(size)], name);

describe("the attachment rules, against the server's", () => {
  it("block the same extensions as the Rust list, in the same order", () => {
    const block = /pub const BLOCKED_EXTENSIONS: \[&str; \d+\] = \[([^\]]*)\]/u.exec(rust)?.[1];
    expect(block).toBeDefined();
    const theirs = [...(block ?? "").matchAll(/"([^"]+)"/gu)].map((m) => m[1]);
    expect(BLOCKED_EXTENSIONS).toEqual(theirs);
  });

  it("allow the same totals", () => {
    expect(rust).toContain(`MAX_FILES: usize = ${String(MAX_ATTACHMENTS)};`);
    expect(drafts).toContain(`MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;`);
    expect(MAX_ATTACHMENT_BYTES).toBe(25 * 1024 * 1024);
  });
});

describe("screen", () => {
  it("reads extensions as the server does", () => {
    expect(extensionOf("../x/Setup.EXE. ")).toBe("exe");
    expect(extensionOf("lease")).toBe("");
    expect(isBlocked("notes.exe.txt")).toBe(false);
    expect(isBlocked("RUN.Bat")).toBe(true);
  });

  it("refuses an empty file, a blocked type and what passes the total", () => {
    const { accepted, refused } = screen(
      [file("a.pdf"), file("empty.pdf", 0), file("setup.EXE"), file("big.bin", 3), file("c.pdf")],
      { bytes: MAX_ATTACHMENT_BYTES - 12, count: 1 },
    );
    expect(accepted.map((f) => f.name)).toEqual(["a.pdf"]);
    expect(refused.map((r) => [r.file.name, r.reason])).toEqual([
      ["empty.pdf", "file_empty"],
      ["setup.EXE", "attachment_blocked"],
      ["big.bin", "attachments_too_large"],
      ["c.pdf", "attachments_too_large"],
    ]);
  });

  it("stops at twenty files", () => {
    const { accepted, refused } = screen([file("a"), file("b")], {
      bytes: 0,
      count: MAX_ATTACHMENTS - 1,
    });
    expect(accepted).toHaveLength(1);
    expect(refused[0]?.reason).toBe("too_many_attachments");
  });

  it("knows the server's refusals", () => {
    expect(refusalOfCode("filename_missing")).toBe("filename_missing");
    expect(refusalOfCode("content_changed")).toBeUndefined();
  });
});

describe("a mailbox that carries less (campbooks-cloud#391)", () => {
  const file = (name: string, size: number) => new File([new Uint8Array(size)], name);

  it("refuses past the mailbox's budget, naming it as the mailbox's limit", () => {
    const { accepted, refused } = screen(
      [file("small.pdf", 1_000_000), file("plans.pdf", 1_500_000)],
      { bytes: 0, count: 0 },
      2_100_000,
    );
    expect(accepted.map((f) => f.name)).toEqual(["small.pdf"]);
    expect(refused).toEqual([
      { file: expect.any(File) as File, reason: "attachments_too_large_for_mailbox" },
    ]);
  });

  it("says the limit as the server does", () => {
    expect(megabytes(2_144_416)).toBe("2.1");
    expect(megabytes(25 * 1024 * 1024)).toBe("25");
  });

  it("knows a slow upload and a cut-short one as refusals", () => {
    expect(refusalOfCode("request_timed_out")).toBe("request_timed_out");
    expect(refusalOfCode("upload_incomplete")).toBe("upload_incomplete");
    expect(refusalOfCode("attachments_too_large_for_mailbox")).toBe(
      "attachments_too_large_for_mailbox",
    );
  });
});
