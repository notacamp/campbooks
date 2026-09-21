/**
 * modules/settings/api/use-integrations-query — integrations data hooks.
 *
 * Three queries:
 *   - overview     GET /api/app/settings/integrations
 *   - notion       GET /api/app/settings/integrations/notion
 *   - calendars    GET /api/app/settings/integrations/calendars
 *   - connections  GET /api/app/settings/integrations/connections
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient, apiCollection } from "~/lib/api";
import type {
  IntegrationsOverviewData,
  NotionIntegrationsData,
  CalendarsData,
  ConnectionData,
} from "~/modules/settings/types";
import type { CollectionEnvelope } from "~/lib/api";

export const integrationsKeys = {
  overview: ["settings", "integrations", "overview"] as const,
  notion: ["settings", "integrations", "notion"] as const,
  calendars: ["settings", "integrations", "calendars"] as const,
  connections: ["settings", "integrations", "connections"] as const,
};

export const useIntegrationsOverviewQuery =
  (): UseQueryResult<IntegrationsOverviewData, Error> =>
    useQuery<IntegrationsOverviewData, Error>({
      queryKey: integrationsKeys.overview,
      queryFn: () =>
        apiClient<IntegrationsOverviewData>("/settings/integrations"),
      staleTime: 2 * 60_000,
    });

export const useNotionIntegrationsQuery =
  (): UseQueryResult<NotionIntegrationsData, Error> =>
    useQuery<NotionIntegrationsData, Error>({
      queryKey: integrationsKeys.notion,
      queryFn: () =>
        apiClient<NotionIntegrationsData>("/settings/integrations/notion"),
      staleTime: 2 * 60_000,
    });

export const useCalendarsIntegrationsQuery =
  (): UseQueryResult<CalendarsData, Error> =>
    useQuery<CalendarsData, Error>({
      queryKey: integrationsKeys.calendars,
      queryFn: () =>
        apiClient<CalendarsData>("/settings/integrations/calendars"),
      staleTime: 2 * 60_000,
    });

export const useConnectionsQuery = (): UseQueryResult<
  CollectionEnvelope<ConnectionData>,
  Error
> =>
  useQuery<CollectionEnvelope<ConnectionData>, Error>({
    queryKey: integrationsKeys.connections,
    queryFn: () =>
      apiCollection<ConnectionData>("/settings/integrations/connections"),
    staleTime: 2 * 60_000,
  });
