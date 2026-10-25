import {
  createRoute,
  useLocation,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { useWorkspaces } from "~/modules/workspaces";

import { MailboxesPage } from "./components/MailboxesPage/MailboxesPage";
import type { UseMailboxConsent } from "./consent";
import { mailboxResult } from "./model";

/** What the front door comes back to Mailboxes with, or `add=1` to open Add a mailbox. */
interface MailboxesSearch {
  connected?: string;
  link_error?: string;
  add?: "1";
}

const text = (value: unknown) => (typeof value === "string" && value !== "" ? value : undefined);

function validateMailboxesSearch(search: Record<string, unknown>): MailboxesSearch {
  const connected = text(search["connected"]);
  const linkError = text(search["link_error"]);
  const add = search["add"] === "1" || search["add"] === 1;
  return {
    ...(connected === undefined ? {} : { connected }),
    ...(linkError === undefined ? {} : { link_error: linkError }),
    ...(add ? { add: "1" as const } : {}),
  };
}

export interface ConnectorsRouteOptions {
  /** The front door: `identity`'s `useMailboxConsent`, handed over by the app shell. */
  useConsent: UseMailboxConsent;
}

/**
 * Settings → Mailboxes, inside a workspace. The result the front door came
 * back with is read once, then cleared from the address bar (a reload
 * shows the list).
 */
export function buildConnectorsRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useConsent }: ConnectorsRouteOptions,
) {
  function MailboxesRoute() {
    const { workspaceId } = useParams({ strict: false });
    const search: MailboxesSearch = useSearch({ strict: false });
    const { pathname } = useLocation();
    const navigate = useNavigate();
    const workspaces = useWorkspaces();
    const consent = useConsent({ workspaceId: workspaceId ?? "", returnTo: pathname });
    const [result] = useState(() => mailboxResult(search));
    const [adding] = useState(() => search.add === "1");
    const hasResult = Object.keys(search).length > 0;
    useEffect(() => {
      if (hasResult) {
        void navigate({ to: pathname, search: {}, replace: true });
      }
    }, [hasResult, navigate, pathname]);
    const workspace = workspaces.data?.find((candidate) => candidate.id === workspaceId);
    // The workspace layout only renders its pages for a workspace in the list.
    return workspace === undefined ? null : (
      <MailboxesPage workspace={workspace} consent={consent} result={result} adding={adding} />
    );
  }

  const mailboxes = createRoute({
    getParentRoute: () => parent,
    path: "/settings/mailboxes",
    validateSearch: validateMailboxesSearch,
    component: MailboxesRoute,
  });
  return [mailboxes] as const;
}
