import { describe, expect, it } from "vitest";

import en from "./i18n/en.json";
import pt from "./i18n/pt-PT.json";
import {
  answerRules,
  answerWho,
  readSender,
  ruleSentence,
  teachable,
  type Translate,
} from "./model";

/** A translator over a catalogue, with i18next's `{{name}}` and `_one/_other`. */
function translator(catalogue: object): Translate {
  return (key, options = {}) => {
    const count = options["count"];
    const plural = typeof count === "number" ? `${key}_${count === 1 ? "one" : "other"}` : key;
    const find = (path: string) =>
      path
        .split(".")
        .reduce<unknown>(
          (node, part) => (node as Record<string, unknown> | undefined)?.[part],
          catalogue,
        );
    const text = (find(plural) ?? find(key)) as string;
    return text.replaceAll(/\{\{(\w+)\}\}/gu, (_, name: string) => String(options[name]));
  };
}

const t = translator(en);

describe("readSender", () => {
  it("reads addresses and domains, and refuses free mail and junk", () => {
    expect(readSender(" Rita@Silva-Contas.example ")).toEqual({
      kind: "address",
      value: "rita@silva-contas.example",
    });
    expect(readSender("@pessoa-obras.example")).toEqual({
      kind: "domain",
      value: "pessoa-obras.example",
    });
    expect(readSender("gmail.com")).toEqual({ kind: "free_mail", value: "gmail.com" });
    expect(readSender("my accountant")).toEqual({ kind: "invalid" });
  });
});

describe("answerRules", () => {
  it("turns each answer into one deterministic rule", () => {
    const rules = answerRules({
      always: ["rita@silva-contas.example", "pessoa-obras.example"],
      never: ["lists", "automatic"],
      neverSenders: ["promo@tilestore.example"],
    });
    expect(rules.map((rule) => [rule.key, rule.conditions, rule.actions])).toEqual([
      [
        "always:rita@silva-contas.example",
        [{ field: "sender_address", value: "rita@silva-contas.example" }],
        [{ action: "surface", value: "always" }],
      ],
      [
        "always:pessoa-obras.example",
        [{ field: "sender_domain", value: "pessoa-obras.example" }],
        [{ action: "surface", value: "always" }],
      ],
      [
        "never:lists",
        [{ field: "header", value: { name: "list_id" } }],
        [{ action: "surface", value: "never" }],
      ],
      [
        "never:automatic",
        [{ field: "header", value: { name: "auto_submitted" } }],
        [{ action: "surface", value: "never" }],
      ],
      [
        "never:promo@tilestore.example",
        [{ field: "sender_address", value: "promo@tilestore.example" }],
        [{ action: "surface", value: "never" }],
      ],
    ]);
  });
});

describe("ruleSentence", () => {
  it("says the common shapes plainly and promises only what a rule does", () => {
    const [always, lists] = answerRules({
      always: ["rita@silva-contas.example"],
      never: ["lists"],
      neverSenders: [],
    });
    if (always === undefined || lists === undefined) {
      throw new Error("two rules expected");
    }
    const names = new Map([["rita@silva-contas.example", "Rita Gomes"]]);
    expect(ruleSentence(always, t, "en", names)).toBe(
      "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.",
    );
    expect(ruleSentence(lists, t, "en")).toBe("Newsletters and mailing lists never need you.");
    expect(ruleSentence(always, translator(pt), "pt-PT")).toBe(
      "O correio de rita@silva-contas.example fica sempre na sua caixa de entrada e nunca é arquivado.",
    );
  });

  it("says any other rule as When …: …", () => {
    expect(
      ruleSentence(
        {
          conditions: [
            { field: "subject_contains", value: "invoice" },
            { field: "sender_domain", value: "tilestore.example" },
          ],
          actions: [{ action: "add_label", label: "label-a" }, { action: "mark_read" }],
        },
        t,
        "en",
        new Map(),
        new Map([["label-a", "Invoices"]]),
      ),
    ).toBe(
      "When the subject contains “invoice” and the sender’s domain is tilestore.example: label it Invoices and mark it read.",
    );
  });
});

describe("answerWho", () => {
  it("adds the sender the member picked to the proposal", () => {
    expect(
      answerWho(
        {
          conditions: [],
          actions: [{ action: "surface", value: "always" }],
          inboxes: [],
          who: { phrase: "my accountant", candidates: [], matched: false },
        },
        "rita@silva-contas.example",
      ),
    ).toEqual({
      conditions: [{ field: "sender_address", value: "rita@silva-contas.example" }],
      actions: [{ action: "surface", value: "always" }],
    });
  });
});

describe("teachable", () => {
  it("takes the sender role", () => {
    expect(teachable({ id: "1", address: "a@example.com", role: "reader", shared: false })).toBe(
      false,
    );
    expect(teachable({ id: "1", address: "a@example.com", role: "sender", shared: true })).toBe(
      true,
    );
  });
});
