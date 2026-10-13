/**
 * A file size for people, in `locale`: "0.4 kB", "277.5 kB", "3.2 MB". Never
 * in bytes, which read as "398 byte" in short form.
 */
export function formatSize(bytes: number, locale: string): string {
  const units = ["kilobyte", "megabyte", "gigabyte"] as const;
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: units[unit],
    unitDisplay: "short",
    maximumFractionDigits: 1,
  }).format(value);
}
