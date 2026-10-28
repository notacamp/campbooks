// What the composer's controls take, apart from the composer itself so the
// menus and the composer can both name them (campbooks-cloud#394).

/** The inboxes the member may send from, when more than one (campbooks-cloud#394). */
export interface ComposerFrom {
  options: readonly { id: string; label: string }[];
  /** The inbox the draft goes from. */
  value: string;
  onChange: (inbox: string) => void;
}

/** What a reply can be switched to. */
export type ComposerMode = "reply" | "reply_all" | "forward";

/** The title's menu: Reply, Reply all, Forward. */
export interface ComposerModes {
  value: ComposerMode;
  /** Resolves once the switch is done (the composer then goes with the old draft). */
  onChange: (mode: ComposerMode) => void;
}

/** What Scout is asked to do with the text. */
export type ScoutRewrite =
  | { kind: "shorter" }
  | { kind: "warmer" }
  | { kind: "spelling" }
  | { kind: "change"; change: string };

/** Scout in the composer (campbooks-cloud#394). */
export interface ComposerScout {
  /** Scout's thinking so far, while it writes. */
  thinking: string;
  /**
   * Ask Scout to rewrite `plain` (the text as plain text). Resolves with its
   * words, or nothing when it failed or was stopped. The composer puts them
   * in as plain text, never as marks or HTML.
   */
  onRewrite: (how: ScoutRewrite, plain: string) => Promise<string | undefined>;
  /**
   * The text was replaced: `undo` puts the previous text back (the screen
   * offers it in a toast; the editor's own undo, ⌘Z, does the same).
   */
  onDone?: ((undo: () => void) => void) | undefined;
}
