/** A value a test knows is there: it fails with a clear message when it is not. */
export function must<TValue>(value: TValue | null | undefined, what = "value"): TValue {
  if (value === null || value === undefined) {
    throw new Error(`expected ${what} to be there`);
  }
  return value;
}
