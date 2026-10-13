// Generates src/lib/api/records.gen.ts from openapi/openapi.json: for each
// record type, the read operations (`x-campbooks.mutates: false`) whose
// `x-campbooks.records` list it. The change feed binding invalidates the
// queries of those operations when a record of that type changes (ADR 0013:
// the OpenAPI document is the one catalogue). Run by `pnpm api:generate`;
// `pnpm api:check` fails when the committed file is stale.
import { readFileSync, writeFileSync } from "node:fs";

interface Meta {
  mutates?: boolean;
  records?: string[];
}

interface Operation {
  operationId?: string;
  "x-campbooks"?: Meta;
}

const specUrl = new URL("../../../openapi/openapi.json", import.meta.url);
const outUrl = new URL("../src/lib/api/records.gen.ts", import.meta.url);

const spec = JSON.parse(readFileSync(specUrl, "utf8")) as {
  paths: Record<string, Record<string, Operation>>;
};

const readers = new Map<string, Set<string>>();
for (const [path, item] of Object.entries(spec.paths)) {
  // The web BFF's own endpoints serve sign-in, not records clients cache.
  if (path.startsWith("/bff/")) {
    continue;
  }
  for (const operation of Object.values(item)) {
    const meta = operation["x-campbooks"];
    const id = operation.operationId;
    if (id === undefined || meta?.mutates !== false) {
      continue;
    }
    for (const record of meta.records ?? []) {
      const ids = readers.get(record) ?? new Set<string>();
      ids.add(id);
      readers.set(record, ids);
    }
  }
}

const entries = [...readers.entries()]
  .toSorted(([a], [b]) => a.localeCompare(b))
  .map(([record, ids]) => {
    const list = [...ids]
      .toSorted((a, b) => a.localeCompare(b))
      .map((id) => JSON.stringify(id))
      .join(", ");
    return `  ${JSON.stringify(record)}: [${list}],`;
  });

const source = `/**
 * Generated from openapi/openapi.json by scripts/records-gen.ts. Do not edit.
 *
 * For each record type, the read operations that return it: a change to a
 * record of that type invalidates the queries keyed by those operationIds.
 */
export const recordReaders: Readonly<Record<string, readonly string[]>> = {
${entries.join("\n")}
};
`;

writeFileSync(outUrl, source);
