// Fails when the UI's messages are not at parity across the shipped locales.
//
// Every directory named `i18n` under src/ that holds JSON messages is one
// namespace (src/lib/i18n is the loader code, not a namespace) and must hold
// one JSON file per locale in SUPPORTED_LOCALES, each with exactly the same keys
// (nested keys compared by path) and no empty strings.
//
//   pnpm i18n:check
import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

import { SUPPORTED_LOCALES } from "../src/lib/i18n/locales.ts";

const root = new URL("../src", import.meta.url).pathname;

interface Messages {
  [key: string]: string | Messages;
}

function namespaceDirs(dir: string): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(dir)) {
    const entryPath = path.join(dir, entry);
    if (!statSync(entryPath).isDirectory()) {
      continue;
    }
    if (entry === "i18n" && readdirSync(entryPath).some((file) => file.endsWith(".json"))) {
      found.push(entryPath);
    } else {
      found.push(...namespaceDirs(entryPath));
    }
  }
  return found;
}

function keyPaths(messages: Messages, prefix = ""): Map<string, string> {
  const paths = new Map<string, string>();
  for (const [key, value] of Object.entries(messages)) {
    const keyPath = prefix === "" ? key : `${prefix}.${key}`;
    if (typeof value === "string") {
      paths.set(keyPath, value);
    } else {
      for (const [nested, text] of keyPaths(value, keyPath)) {
        paths.set(nested, text);
      }
    }
  }
  return paths;
}

const problems: string[] = [];
const dirs = namespaceDirs(root);

for (const dir of dirs) {
  const name = path.relative(root, dir);
  const files = new Set(readdirSync(dir).filter((file) => file.endsWith(".json")));
  const byLocale = new Map<string, Map<string, string>>();

  for (const locale of SUPPORTED_LOCALES) {
    const file = `${locale}.json`;
    if (!files.has(file)) {
      problems.push(`${name}: missing ${file}`);
      continue;
    }
    files.delete(file);
    const messages = JSON.parse(readFileSync(path.join(dir, file), "utf8")) as Messages;
    byLocale.set(locale, keyPaths(messages));
  }
  for (const extra of files) {
    problems.push(`${name}: ${extra} is not a supported locale (${SUPPORTED_LOCALES.join(", ")})`);
  }

  const reference = byLocale.get("en");
  if (reference === undefined) {
    continue;
  }
  for (const [locale, keys] of byLocale) {
    for (const [key, text] of keys) {
      if (text.trim() === "") {
        problems.push(`${name}/${locale}.json: "${key}" is empty`);
      }
      if (locale !== "en" && !reference.has(key)) {
        problems.push(`${name}/${locale}.json: "${key}" is not in en.json`);
      }
    }
    if (locale === "en") {
      continue;
    }
    for (const key of reference.keys()) {
      if (!keys.has(key)) {
        problems.push(`${name}/${locale}.json: missing "${key}"`);
      }
    }
  }
}

if (problems.length > 0) {
  console.error(`i18n parity: ${String(problems.length)} problem(s)\n  ${problems.join("\n  ")}`);
  process.exitCode = 1;
} else {
  console.log(
    `i18n parity: ${String(dirs.length)} namespaces × ${String(SUPPORTED_LOCALES.length)} locales OK`,
  );
}
