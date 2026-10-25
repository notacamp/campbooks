/** The code languages a page may name: the server's list, and nothing else. */
const CODE_LANGUAGES = [
  "bash",
  "c",
  "cpp",
  "csharp",
  "css",
  "go",
  "html",
  "java",
  "javascript",
  "json",
  "kotlin",
  "markdown",
  "php",
  "python",
  "ruby",
  "rust",
  "sql",
  "swift",
  "typescript",
  "xml",
  "yaml",
  "plaintext",
] as const;

const ALIASES: Readonly<Record<string, (typeof CODE_LANGUAGES)[number]>> = {
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  "c++": "cpp",
  "c#": "csharp",
  cs: "csharp",
  golang: "go",
  js: "javascript",
  jsx: "javascript",
  md: "markdown",
  py: "python",
  rb: "ruby",
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  yml: "yaml",
  text: "plaintext",
  txt: "plaintext",
};

/** A language a person typed, as one the server accepts; `null` when it is not one. */
export function codeLanguage(typed?: string | null): string | null {
  const name = (typed ?? "").trim().toLowerCase();
  if (name === "") {
    return null;
  }
  return CODE_LANGUAGES.find((language) => language === name) ?? ALIASES[name] ?? null;
}
