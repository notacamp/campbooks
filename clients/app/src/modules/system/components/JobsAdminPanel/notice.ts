/** What the last action said, shown above the Jobs page until dismissed. */
export interface Notice {
  tone: "neutral" | "danger";
  text: string;
}
