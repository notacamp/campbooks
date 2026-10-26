import {
  createRoute,
  useLocation,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { mailboxResult, type UseMailboxConsent } from "~/modules/connectors";
import type { UseTaskSources } from "~/modules/tasks";

import { CalendarScreen, type ScheduleSearch } from "./components/Calendar/CalendarScreen";
import { CalendarSettings } from "./components/Settings/CalendarSettings";
import { isView, isYmd } from "./model";

/** The address holds only what it understands: a view, a day, an event. */
function validateScheduleSearch(search: Record<string, unknown>): ScheduleSearch {
  const { view, date, event } = search;
  return {
    ...(isView(view) ? { view } : {}),
    ...(isYmd(date) ? { date } : {}),
    ...(typeof event === "string" && event !== "" ? { event } : {}),
  };
}

/** What the front door comes back to Settings → Calendar with. */
interface SettingsSearch {
  connected?: string;
  link_error?: string;
}

const text = (value: unknown) => (typeof value === "string" && value !== "" ? value : undefined);

function validateSettingsSearch(search: Record<string, unknown>): SettingsSearch {
  const connected = text(search["connected"]);
  const linkError = text(search["link_error"]);
  return {
    ...(connected === undefined ? {} : { connected }),
    ...(linkError === undefined ? {} : { link_error: linkError }),
  };
}

export interface ScheduleRouteOptions {
  /** The front door, `identity`'s `useMailboxConsent`, handed over by the app shell. */
  useConsent: UseMailboxConsent;
  /** Where tasks made from conversations came from, `conversations`' `useConversationSources`. */
  useSources: UseTaskSources;
}

/** The module's pages; in the app, `parent` is the workspace layout (`/w/$workspaceId`). */
export function buildScheduleRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useConsent, useSources }: ScheduleRouteOptions,
) {
  function CalendarRoute() {
    const { workspaceId } = useParams({ strict: false });
    const search: ScheduleSearch = useSearch({ strict: false });
    const navigate = useNavigate();
    return (
      <CalendarScreen
        workspaceId={workspaceId ?? ""}
        search={search}
        useSources={useSources}
        onSearch={(next) => {
          void navigate({
            to: "/w/$workspaceId/schedule",
            params: { workspaceId: workspaceId ?? "" },
            search: next,
          });
        }}
      />
    );
  }

  function SettingsRoute() {
    const { workspaceId } = useParams({ strict: false });
    const search: SettingsSearch = useSearch({ strict: false });
    const { pathname } = useLocation();
    const navigate = useNavigate();
    const consent = useConsent({ workspaceId: workspaceId ?? "", returnTo: pathname });
    // What the front door came back with is read once, then cleared from the address bar.
    const [result] = useState(() => mailboxResult(search));
    const hasResult = Object.keys(search).length > 0;
    useEffect(() => {
      if (hasResult) {
        void navigate({ to: pathname, search: {}, replace: true });
      }
    }, [hasResult, navigate, pathname]);
    return <CalendarSettings workspaceId={workspaceId ?? ""} consent={consent} result={result} />;
  }

  const home = createRoute({
    getParentRoute: () => parent,
    path: "/schedule",
    validateSearch: validateScheduleSearch,
    component: CalendarRoute,
  });
  const settings = createRoute({
    getParentRoute: () => parent,
    path: "/settings/calendar",
    validateSearch: validateSettingsSearch,
    component: SettingsRoute,
  });
  return [home, settings] as const;
}
