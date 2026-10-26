import { describe, expect, it } from "vitest";

import {
  columnsOf,
  conditionReady,
  configJson,
  dateJson,
  dateOf,
  dayOf,
  decimalOf,
  editable,
  filesOf,
  filterOf,
  filterable,
  flatFilter,
  formatDate,
  formatInstant,
  formatNumber,
  formatSummary,
  groupable,
  hrefOf,
  idsOf,
  isEmptyValue,
  joinWhen,
  linksOf,
  nextColor,
  operatorsFor,
  optionById,
  optionsOf,
  plainText,
  runsOf,
  safeHref,
  showsWho,
  sortable,
  specsOf,
  startingValue,
  summaryKey,
  summaryKinds,
  takesValue,
  timeOf,
  viewConfig,
} from "./model";
import type { FilterNode, PropertyView } from "./types";

const property = (over: Partial<PropertyView>): PropertyView => ({
  id: "p1",
  name: "Property",
  type: "text",
  config: {},
  position: "a0",
  can_revert: false,
  ...over,
});

describe("a view's config", () => {
  it("reads what fits and leaves out what does not", () => {
    const config = viewConfig({
      config: {
        filter: { and: [{ property: "p1", operator: "is", value: "x" }, { operator: "is" }] },
        sorts: [
          { property: "p1", direction: "desc" },
          { property: "p2", direction: "sideways" },
          3,
        ],
        summaries: [
          { property: "p1", kind: "sum" },
          { property: "p1", kind: "median" },
        ],
        properties: [{ property: "p1", visible: false, width: 200 }, { visible: true }, "x"],
        group_by: "p3",
        hidden_groups: ["a", 4],
        card_properties: ["p1"],
      },
    });
    expect(config.filter).toEqual({ and: [{ property: "p1", operator: "is", value: "x" }] });
    expect(config.sorts).toEqual([{ property: "p1", direction: "desc" }]);
    expect(config.summaries).toEqual([{ property: "p1", kind: "sum" }]);
    expect(config.properties).toEqual([{ property: "p1", visible: false, width: 200 }]);
    expect(config.groupBy).toBe("p3");
    expect(config.hiddenGroups).toEqual(["a"]);
    expect(config.cardProperties).toEqual(["p1"]);
  });

  it("is empty for nothing, and for something that is not an object", () => {
    for (const config of [null, undefined, "x", [1]]) {
      expect(viewConfig({ config })).toEqual({
        filter: undefined,
        sorts: [],
        summaries: [],
        properties: [],
        hiddenGroups: [],
        cardProperties: [],
      });
    }
  });

  it("is written back without empty parts, and a board keeps its groups", () => {
    const config = viewConfig({
      config: {
        filter: { and: [] },
        sorts: [{ property: "p1", direction: "asc" }],
        properties: [{ property: "p1", width: 90 }],
        group_by: "p2",
        hidden_groups: ["none"],
      },
    });
    expect(configJson(config, false)).toEqual({
      sorts: [{ property: "p1", direction: "asc" }],
      properties: [{ property: "p1", width: 90 }],
    });
    expect(configJson(config, true)).toEqual({
      sorts: [{ property: "p1", direction: "asc" }],
      properties: [{ property: "p1", width: 90 }],
      group_by: "p2",
      hidden_groups: ["none"],
    });
  });
});

describe("filters", () => {
  const condition = (n: number): FilterNode => ({
    property: `p${String(n)}`,
    operator: "is",
    value: n,
  });

  it("flattens one group of conditions, and says when it is nested", () => {
    expect(flatFilter()).toEqual({ mode: "and", conditions: [], nested: false });
    expect(flatFilter(condition(1))).toMatchObject({ conditions: [condition(1)], nested: false });
    expect(flatFilter({ or: [condition(1), condition(2)] })).toMatchObject({
      mode: "or",
      conditions: [condition(1), condition(2)],
      nested: false,
    });
    // A group of one is that condition; a group of two stays a group.
    expect(flatFilter({ and: [{ or: [condition(1)] }, condition(2)] })).toMatchObject({
      conditions: [condition(1), condition(2)],
      nested: false,
    });
    expect(flatFilter({ and: [{ or: [condition(1), condition(2)] }, condition(3)] })).toMatchObject(
      {
        nested: true,
      },
    );
    expect(flatFilter({ and: [{ or: [] }, condition(3)] })).toMatchObject({ nested: false });
  });

  it("makes the filter of a flat one, or none", () => {
    expect(filterOf({ mode: "and", conditions: [] })).toBeUndefined();
    expect(filterOf({ mode: "or", conditions: [condition(1) as never] })).toEqual({
      or: [condition(1)],
    });
  });

  it("knows which operators a type accepts and which need a value", () => {
    expect(operatorsFor(property({ type: "select" }))).toContain("is_any_of");
    expect(operatorsFor(property({ type: "checkbox" }))).toEqual(["is"]);
    expect(operatorsFor(property({ type: "files" }))).toEqual(["is_empty", "is_not_empty"]);
    expect(operatorsFor(property({ type: "created", config: { of: "who" } }))).toEqual([
      "is",
      "is_not",
    ]);
    expect(operatorsFor(property({ type: "edited", config: { of: "time" } }))).toContain("before");
    for (const type of ["title", "number", "multi_select", "date", "campbooks_link"] as const) {
      expect(operatorsFor(property({ type })).length).toBeGreaterThan(0);
    }
    expect(filterable(property({ type: "notion_formula" }))).toBe(false);
    expect(filterable(property({ type: "text" }))).toBe(true);
    expect(takesValue("is_empty")).toBe(false);
    expect(takesValue("contains")).toBe(true);
    expect(showsWho(property({ type: "created", config: { of: "who" } }))).toBe(true);
  });

  it("starts a condition with a value that fits", () => {
    const select = property({
      type: "select",
      config: { options: [{ id: "o1", name: "A", color: "red" }] },
    });
    expect(startingValue(select, "is")).toBe("o1");
    expect(startingValue(select, "is_any_of")).toEqual([]);
    expect(startingValue(property({ type: "checkbox" }), "is")).toBe(true);
    expect(startingValue(property({ type: "text" }), "contains")).toBe("");
    expect(startingValue(property({ type: "text" }), "is_empty")).toBeUndefined();
    expect(startingValue(property({ type: "campbooks_link" }), "contains")).toBeUndefined();
  });

  it("sends only conditions that are complete", () => {
    expect(conditionReady({ property: "p", operator: "is_empty" })).toBe(true);
    expect(conditionReady({ property: "p", operator: "is", value: "  " })).toBe(false);
    expect(conditionReady({ property: "p", operator: "is", value: "a" })).toBe(true);
    expect(conditionReady({ property: "p", operator: "is_any_of", value: [] })).toBe(false);
    expect(conditionReady({ property: "p", operator: "is_any_of", value: ["a"] })).toBe(true);
    expect(conditionReady({ property: "p", operator: "is", value: false })).toBe(true);
    expect(conditionReady({ property: "p", operator: "is" })).toBe(false);
  });
});

describe("types", () => {
  it("lists the summaries a type can show", () => {
    expect(summaryKinds("text")).toEqual(["count"]);
    expect(summaryKinds("number")).toEqual(["count", "sum", "average", "min", "max"]);
    expect(summaryKinds("checkbox")).toEqual(["count", "percent_checked"]);
    expect(summaryKinds("date")).toEqual(["count", "min", "max"]);
    expect(summaryKey("percent_checked")).toBe("percentChecked");
    expect(summaryKey("sum")).toBe("sum");
  });

  it("knows what can be sorted, grouped and written", () => {
    expect(sortable(property({ type: "number" }))).toBe(true);
    expect(sortable(property({ type: "relation" }))).toBe(false);
    expect(sortable(property({ type: "created", config: { of: "who" } }))).toBe(false);
    expect(sortable(property({ type: "created", config: { of: "time" } }))).toBe(true);
    expect(groupable("select")).toBe(true);
    expect(groupable("multi_select")).toBe(false);
    expect(editable("text")).toBe(true);
    for (const type of [
      "title",
      "created",
      "edited",
      "unique_id",
      "external",
      "notion_rollup",
    ] as const) {
      expect(editable(type)).toBe(false);
    }
  });
});

describe("options", () => {
  const select = property({
    type: "status",
    config: {
      options: [
        { id: "a", name: "To do", color: "gray", group: "todo" },
        { id: "b", name: "Done", color: "chartreuse" },
        { name: "No id" },
        "junk",
      ],
    },
  });

  it("reads them with a colour and a group, and ignores what is not one", () => {
    expect(optionsOf(select)).toEqual([
      { id: "a", name: "To do", color: "gray", group: "todo" },
      { id: "b", name: "Done", color: "default" },
    ]);
    expect(optionsOf(property({ config: null }))).toEqual([]);
    expect(optionById(select, "b")?.name).toBe("Done");
    expect(optionById(select)).toBeUndefined();
  });

  it("takes the next colour round the palette", () => {
    expect(nextColor([])).toBe("gray");
    expect(nextColor(optionsOf(select))).toBe("orange");
  });
});

describe("values", () => {
  it("reads text runs and writes plain ones", () => {
    expect(plainText([{ text: "Hello " }, { text: "there", href: "https://x.example" }])).toBe(
      "Hello there",
    );
    expect(plainText("nope")).toBe("");
    expect(runsOf("  Hi  ")).toEqual([{ text: "Hi" }]);
    expect(runsOf(" ".repeat(3))).toBeNull();
  });

  it("reads ids, links, files and dates", () => {
    expect(idsOf("a")).toEqual(["a"]);
    expect(idsOf(["a", 3, "b"])).toEqual(["a", "b"]);
    expect(idsOf(null)).toEqual([]);
    expect(linksOf([{ type: "tasks.task", id: "t" }, { type: "x" }, 3])).toEqual([
      { type: "tasks.task", id: "t" },
    ]);
    const file = { hash: "h", name: "a.pdf", size: 3, type: "application/pdf" };
    expect(filesOf([file, { name: "x" }])).toEqual([file]);
    expect(dateOf({ start: "2026-10-30", end: "2026-11-02", time_zone: "UTC" })).toEqual({
      start: "2026-10-30",
      end: "2026-11-02",
      timeZone: "UTC",
    });
    expect(dateOf({ end: "x" })).toBeUndefined();
    expect(dateOf(null)).toBeUndefined();
    expect(dateJson({ start: "2026-10-30", end: undefined, timeZone: undefined })).toEqual({
      start: "2026-10-30",
    });
    expect(dateJson({ start: "a", end: "b", timeZone: "c" })).toEqual({
      start: "a",
      end: "b",
      time_zone: "c",
    });
  });

  it("splits and joins a day and a time of day", () => {
    expect(dayOf("2026-10-30")).toBe("2026-10-30");
    expect(timeOf("2026-10-30")).toBeUndefined();
    const joined = joinWhen("2026-10-30", "09:30");
    expect(joined).toMatch(/^2026-10-30T|^2026-10-29T|^2026-10-31T/u);
    expect(joined.endsWith("Z")).toBe(true);
    expect(dayOf(joined)).toBe("2026-10-30");
    expect(timeOf(joined)).toBe("09:30");
    expect(joinWhen("2026-10-30")).toBe("2026-10-30");
    expect(joinWhen("nope", "09:30")).toBe("nope");
    expect(dayOf("2026-13-45T25:00:00Z")).toBe("2026-13-45");
    expect(timeOf("garbageTgarbage")).toBeUndefined();
  });

  it("knows an empty value from a false one", () => {
    for (const value of [null, undefined, "", []]) {
      expect(isEmptyValue(value)).toBe(true);
    }
    for (const value of [false, 0, "a", [1]]) {
      expect(isEmptyValue(value)).toBe(false);
    }
  });

  it("reads a typed number as the server's decimal", () => {
    expect(decimalOf(" 12,5 ")).toBe("12.5");
    expect(decimalOf("-3")).toBe("-3");
    for (const bad of ["", "abc", "1e3", "1.2345678901", "12345678901234567"]) {
      expect(decimalOf(bad)).toBeUndefined();
    }
  });
});

describe("how values are shown", () => {
  it("formats numbers in their format and language", () => {
    expect(
      formatNumber("9600", property({ config: { format: "currency", currency: "EUR" } }), "en"),
    ).toBe("€9,600.00");
    expect(formatNumber("9600", property({ config: { format: "number" } }), "pt-PT")).toBe("9600");
    expect(formatNumber("42", property({ config: { format: "percent" } }), "en")).toBe("42 %");
    expect(formatNumber(7, property({ config: {} }), "en")).toBe("7");
    expect(formatNumber("x", property({ config: {} }), "en")).toBe("");
    expect(formatNumber(null, property({ config: {} }), "en")).toBe("");
    expect(
      formatNumber("5", property({ config: { format: "currency", currency: "??" } }), "en"),
    ).toBe("5");
  });

  it("formats dates and instants", () => {
    expect(formatDate({ start: "2026-10-30" }, "en")).toContain("30");
    expect(formatDate({ start: "2026-10-30", end: "2026-11-02" }, "en")).toContain("→");
    expect(formatDate(null, "en")).toBe("");
    expect(formatDate({ start: "garbage" }, "en")).toBe("garbage");
    expect(formatDate({ start: "2026-10-30T09:30:00Z" }, "en")).toMatch(/\d/u);
    expect(formatInstant("2026-10-30T09:30:00Z", "en")).toContain("2026");
    expect(formatInstant("nope", "en")).toBe("nope");
  });

  it("formats a summary in its property's format", () => {
    const value = property({ type: "number", config: { format: "currency", currency: "EUR" } });
    expect(formatSummary("sum", "28400", value, "en")).toBe("€28,400.00");
    expect(formatSummary("count", "4", value, "en")).toBe("4");
    expect(formatSummary("percent_checked", "50", property({ type: "checkbox" }), "en")).toBe(
      "50 %",
    );
    expect(formatSummary("max", "2026-10-30", property({ type: "date" }), "en")).toContain("30");
    expect(
      formatSummary("max", "2026-10-30T09:30:00Z", property({ type: "created" }), "en"),
    ).toContain("2026");
    expect(formatSummary("sum", null, value, "en")).toBe("");
  });
});

describe("links", () => {
  it("only lets https, http and mailto through", () => {
    expect(safeHref("https://example.com/a")).toBe("https://example.com/a");
    expect(safeHref("http://example.com")).toBe("http://example.com/");
    expect(safeHref("mailto:ana@example.com")).toBe("mailto:ana@example.com");
    for (const bad of [
      "javascript:alert(1)",
      "data:text/html,x",
      "tel:123",
      "ftp://x.example",
      "example.com",
      "",
      "https://a b.example",
      "vbscript:x",
    ]) {
      expect(safeHref(bad)).toBeUndefined();
    }
  });

  it("makes the address of a url or email cell, and nothing for the rest", () => {
    expect(hrefOf("url", "https://example.com")).toBe("https://example.com/");
    expect(hrefOf("url", "javascript:alert(1)")).toBeUndefined();
    expect(hrefOf("email", "ana@example.com")).toBe("mailto:ana@example.com");
    expect(hrefOf("email", "not an address")).toBeUndefined();
    expect(hrefOf("phone", "+351 912")).toBeUndefined();
    expect(hrefOf("url", 5)).toBeUndefined();
  });
});

describe("columns", () => {
  const title = property({ id: "t", type: "title", name: "Name", position: "a0" });
  const number = property({ id: "n", type: "number", name: "Value", position: "a2" });
  const select = property({ id: "s", type: "select", name: "Stage", position: "a1" });

  it("follow the view, then any property it does not name, with the title first", () => {
    const config = viewConfig({
      config: {
        properties: [
          { property: "n", width: 100 },
          { property: "s", visible: false },
          { property: "gone" },
          { property: "n" },
        ],
      },
    });
    const columns = columnsOf([number, select, title], config);
    expect(columns.map((column) => column.property.id)).toEqual(["t", "n", "s"]);
    expect(columns.map((column) => column.visible)).toEqual([true, true, false]);
    expect(columns.find((column) => column.property.id === "n")?.width).toBe(100);
    expect(specsOf(columns)).toEqual([
      { property: "t", visible: true, width: columns[0]?.width },
      { property: "n", visible: true, width: 100 },
      { property: "s", visible: false, width: columns[2]?.width },
    ]);
  });

  it("keep a width between the narrowest and the widest, and the title always shows", () => {
    const config = viewConfig({
      config: {
        properties: [
          { property: "n", width: 5 },
          { property: "t", visible: false, width: 5000 },
        ],
      },
    });
    const columns = columnsOf([title, number], config);
    expect(columns.find((column) => column.property.id === "n")?.width).toBe(64);
    expect(columns.find((column) => column.property.id === "t")?.width).toBe(640);
    expect(columns.find((column) => column.property.id === "t")?.visible).toBe(true);
    expect(columnsOf([title, number], viewConfig({ config: {} })).map((c) => c.width)).toEqual([
      240, 128,
    ]);
  });
});
