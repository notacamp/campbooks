/** Standard base64 of `bytes`, as the server reads and writes it. */
export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  const step = 0x80_00;
  for (let at = 0; at < bytes.length; at += step) {
    binary += String.fromCodePoint(...bytes.subarray(at, at + step));
  }
  return btoa(binary);
}

/** The bytes of standard base64 `text`. */
export function fromBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let at = 0; at < binary.length; at += 1) {
    bytes[at] = binary.codePointAt(at) ?? 0;
  }
  return bytes;
}
