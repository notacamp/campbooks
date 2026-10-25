import { useEffect, useRef, useState } from "react";
import * as Y from "yjs";

import { errorStatus } from "~/lib/api";

import type { usePageCalls } from "../api/queries";

import { fromBase64 } from "./bytes";
import { PageSync, type SaveState } from "./sync";

export interface PageDocumentInput {
  pageId: string;
  /** The body as the server sent it with the page, once, when it opens. */
  update: string;
  stateVector: string;
  /** Grows when the page changed elsewhere (the change feed refetched it). */
  revision: number;
  calls: ReturnType<typeof usePageCalls>;
}

/**
 * A page's live Yjs document and how it stands with the server. Open a
 * page with its own `key`: the document is made once per page. Typing is
 * sent through `pages.sync`; when the page's revision moves (the change
 * feed), what others wrote is pulled in.
 */
export function usePageDocument({
  pageId,
  update,
  stateVector,
  revision,
  calls,
}: PageDocumentInput) {
  const [session] = useState(() => {
    const doc = new Y.Doc();
    Y.applyUpdate(doc, fromBase64(update));
    return { doc, serverVector: fromBase64(stateVector) };
  });
  const [state, setState] = useState<SaveState>("saved");
  const sync = useRef<PageSync | undefined>(undefined);

  useEffect(() => {
    const synchroniser = new PageSync(
      session.doc,
      session.serverVector,
      (body) => calls.sync(pageId, body),
      // A 400 is the server refusing the change itself; retrying cannot help.
      { refused: (error) => errorStatus(error) === 400 },
    );
    sync.current = synchroniser;
    const stop = synchroniser.subscribe(setState);
    return () => {
      stop();
      sync.current = undefined;
      // What was typed last goes out as the page closes.
      void synchroniser.flush().finally(() => {
        synchroniser.stop();
      });
    };
  }, [calls, pageId, session]);

  const seen = useRef(revision);
  useEffect(() => {
    if (revision !== seen.current) {
      seen.current = revision;
      void sync.current?.pull();
    }
  }, [revision]);

  return { doc: session.doc, state };
}
