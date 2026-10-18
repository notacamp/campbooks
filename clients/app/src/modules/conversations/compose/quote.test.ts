import { describe, expect, it } from "vitest";

import { Q4_DETAIL } from "../../../../test/demo/conversations";

import { addressLine, displayName, forwardQuote, replyQuote, type QuoteWords } from "./quote";

const words: QuoteWords = {
  wrote: (date, name) => `On ${date}, ${name} wrote:`,
  forwarded: "Forwarded message",
  from: "From",
  date: "Date",
  subject: "Subject",
  to: "To",
};

const [first] = Q4_DETAIL.messages;
// The member's own time zone: whatever the test machine's is.
const sent = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(
  new Date("2026-10-11T11:00:00Z"),
);

describe("quotes", () => {
  it("quotes a reply line by line under an attribution", () => {
    if (first === undefined) {
      throw new Error("no message");
    }
    const quote = replyQuote(
      { ...first, text: { content: "Hi Sam,\n\nCould you?", truncated: false } },
      "en",
      words,
    );
    expect(quote).toBe(`On ${sent}, Maya Okafor wrote:\n> Hi Sam,\n>\n> Could you?`);
  });

  it("heads a forward with who, when and what", () => {
    if (first === undefined) {
      throw new Error("no message");
    }
    const quote = forwardQuote(first, "en", words);
    expect(quote.split("\n").slice(0, 5)).toEqual([
      "---------- Forwarded message ----------",
      "From: Maya Okafor <maya@northwind-studio.example>",
      `Date: ${sent}`,
      "Subject: Q4 plan: can you review by Friday?",
      "To: Sam Rivera <sam.rivera@example.com>",
    ]);
    expect(quote).toContain("Could you review it by Friday?");
    expect(
      forwardQuote({ ...first, text: null }, "en", words).endsWith(
        "To: Sam Rivera <sam.rivera@example.com>",
      ),
    ).toBe(true);
  });

  it("quotes the words of an HTML-only message, as the server reads them (#218)", () => {
    if (first === undefined) {
      throw new Error("no message");
    }
    // `conversations.get` gives an HTML-only message its HTML's words as
    // its text; a reply and a forward quote them.
    const htmlOnly = {
      ...first,
      has_html: true,
      text: {
        content: "Hello Sam,\nYour berth at the harbourlight pontoon is kept.\n",
        truncated: false,
      },
    };
    expect(replyQuote(htmlOnly, "en", words)).toContain("> Your berth at the harbourlight pontoon");
    expect(forwardQuote(htmlOnly, "en", words)).toContain(
      "Your berth at the harbourlight pontoon is kept.",
    );
  });

  it("names people by name, else address", () => {
    expect(displayName({ address: "x@example.com" })).toBe("x@example.com");
    expect(displayName({ name: " ", address: "x@example.com" })).toBe("x@example.com");
    expect(addressLine({ name: "X", address: "x@example.com" })).toBe("X <x@example.com>");
  });
});
