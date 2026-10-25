import { createContext, use } from "react";

/** What the blocks of one open page need to know about where they are. */
export interface EditorEnvironment {
  workspaceId: string;
  pageId: string;
  /** Open an address in the app without a reload. */
  open: (href: string) => void;
}

export const EditorEnvironmentContext = createContext<EditorEnvironment | null>(null);

/** The open page's environment; blocks only render inside the editor. */
export function useEditorEnvironment(): EditorEnvironment {
  const environment = use(EditorEnvironmentContext);
  if (environment === null) {
    throw new Error("useEditorEnvironment() must be used inside the page editor");
  }
  return environment;
}
