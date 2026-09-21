/**
 * modules/settings/mailboxes/hooks — TanStack Query hooks for mailbox
 * connect and management.
 *
 * These hooks call the /api/app endpoints via apiClient (never fetch directly).
 */
import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationResult,
  type UseQueryResult,
} from "@tanstack/react-query";
import { apiClient, apiCollection } from "~/lib/api";
import type {
  AuthorizeUrlResponse,
  ConnectedMailbox,
  ImapConnectParams,
  MailboxProvider,
} from "./types";

// ── Query keys ────────────────────────────────────────────────────────────────

export const mailboxKeys = {
  all: ["mailboxes"] as const,
  list: () => [...mailboxKeys.all, "list"] as const,
  authorizeUrl: (provider: MailboxProvider) =>
    [...mailboxKeys.all, "authorizeUrl", provider] as const,
};

// ── List connected mailboxes ──────────────────────────────────────────────────

export const useMailboxesQuery = (): UseQueryResult<ConnectedMailbox[], Error> =>
  useQuery({
    queryKey: mailboxKeys.list(),
    queryFn: async () => {
      const { data } = await apiCollection<ConnectedMailbox>("/email_accounts");
      return data;
    },
    staleTime: 30_000,
  });

// ── OAuth authorize URL ───────────────────────────────────────────────────────

/**
 * Lazy hook: only fetches when `enabled` is true.
 * Callers enable it on user intent (e.g. clicking the provider card).
 * The returned authorize_url is opened in the same tab so the browser
 * follows the OAuth redirect and lands back at return_to.
 */
export const useAuthorizeUrlQuery = (
  provider: MailboxProvider | null,
  { enabled = false }: { enabled?: boolean } = {},
): UseQueryResult<AuthorizeUrlResponse, Error> =>
  useQuery({
    queryKey: mailboxKeys.authorizeUrl(provider ?? ("" as MailboxProvider)),
    queryFn: async () =>
      apiClient<AuthorizeUrlResponse>(
        `/oauth/authorize_url?${new URLSearchParams({
          provider: provider!,
          return_to: `${window.location.origin}/settings/mailboxes?connected=${provider}`,
        })}`,
      ),
    enabled: enabled && provider !== null,
    staleTime: 5 * 60_000, // 5 min — state token lasts 30 min
    gcTime: 5 * 60_000,
  });

// ── Disconnect a mailbox ──────────────────────────────────────────────────────

export const useDisconnectMutation = (): UseMutationResult<
  void,
  Error,
  number,
  { previous: ConnectedMailbox[] | undefined }
> => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => {
      await apiClient<void>(`/email_accounts/${id}`, { method: "DELETE" });
    },
    // Optimistic update: remove from the list immediately.
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey: mailboxKeys.list() });
      const previous = queryClient.getQueryData<ConnectedMailbox[]>(mailboxKeys.list());
      queryClient.setQueryData<ConnectedMailbox[]>(
        mailboxKeys.list(),
        (old) => old?.filter((m) => m.id !== id) ?? [],
      );
      return { previous };
    },
    onError: (_err, _id, context) => {
      if (context?.previous) {
        queryClient.setQueryData(mailboxKeys.list(), context.previous);
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: mailboxKeys.list() });
    },
  });
};

// ── Connect an IMAP mailbox ───────────────────────────────────────────────────

export const useImapConnectMutation = (): UseMutationResult<
  ConnectedMailbox,
  Error,
  ImapConnectParams
> => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: ImapConnectParams) =>
      apiClient<ConnectedMailbox>("/imap_accounts", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: mailboxKeys.list() });
    },
  });
};
