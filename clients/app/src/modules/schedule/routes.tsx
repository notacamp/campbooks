import {
  createRoute,
  useLocation,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";

import type { UseMailboxConsent } from "~/modules/connectors";

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

export interface ScheduleRouteOptions {
  /** The front door, `identity`'s `useMailboxConsent`, handed over by the app shell. */
  useConsent: UseMailboxConsent;
}

/** The module's pages; in the app, `parent` is the workspace layout (`/w/$workspaceId`). */
export function buildScheduleRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useConsent }: ScheduleRouteOptions,
) {
  function CalendarRoute() {
    const { workspaceId } = useParams({ strict: false });
    const search: ScheduleSearch = useSearch({ strict: false });
    const navigate = useNavigate();
    return (
      <CalendarScreen
        workspaceId={workspaceId ?? ""}
        search={search}
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
    const { pathname } = useLocation();
    const consent = useConsent({ workspaceId: workspaceId ?? "", returnTo: pathname });
    return <CalendarSettings workspaceId={workspaceId ?? ""} consent={consent} />;
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
    component: SettingsRoute,
  });
  return [home, settings] as const;
}
