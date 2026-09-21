/**
 * IntegrationsPage — Settings → Integrations at /settings/integrations.
 *
 * Shows connection status for:
 *   - Google Drive (status only — connect via OAuth redirect)
 *   - Notion (status + list of connected workspaces + manual token connect
 *             for self-hosted + disconnect per workspace)
 *   - Zoho Drive (status only)
 *   - Calendars (list of connected calendar accounts + per-calendar sync status)
 *   - Connections (CRUD for custom HTTP connections used by workflows)
 *
 * OAuth connect flows (Drive, Notion OAuth, Zoho) are handled by the Rails
 * OAuth controllers; this page shows status and provides disconnect + manual
 * token connect where available.
 */
import { type FC, useState, useCallback, useRef } from "react";
import {
  RotateCcw,
  Check,
  X,
  Plus,
  ExternalLink,
  Calendar,
  Plug,
  Pencil,
  Trash2,
} from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useIntegrationsOverviewQuery,
  useNotionIntegrationsQuery,
  useCalendarsIntegrationsQuery,
  useConnectionsQuery,
  useNotionConnectMutation,
  useNotionDisconnectMutation,
  useCreateConnectionMutation,
  useUpdateConnectionMutation,
  useDeleteConnectionMutation,
} from "~/modules/settings/api";
import type {
  NotionIntegration,
  CalendarAccountData,
  ConnectionData,
} from "~/modules/settings/types";
import { cn } from "~/lib/utils";

// ── Shared helpers ────────────────────────────────────────────────────────────

const FieldError: FC<{ error: Error | null }> = ({ error }) => {
  if (!error) return null;
  const msg = error instanceof ApiError ? error.message : "Something went wrong";
  return (
    <p className="text-[12px] text-danger mt-1.5" role="alert">
      {msg}
    </p>
  );
};

const StatusDot: FC<{ connected: boolean }> = ({ connected }) => (
  <span
    className={cn(
      "inline-block w-2 h-2 rounded-full shrink-0",
      connected ? "bg-money-text" : "bg-t4",
    )}
    aria-hidden="true"
  />
);

// ── StatusCard — reusable connect-status card ─────────────────────────────────

interface StatusCardProps {
  name: string;
  connected: boolean;
  detail?: string;
  connectUrl?: string;
  children?: React.ReactNode;
}

const StatusCard: FC<StatusCardProps> = ({
  name,
  connected,
  detail,
  connectUrl,
  children,
}) => (
  <div className="py-4 border-b border-line last:border-b-0">
    <div className="flex items-center gap-3">
      <StatusDot connected={connected} />
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-[500] text-t1">{name}</p>
        {detail && (
          <p className="text-[12px] text-t3 mt-0.5">{detail}</p>
        )}
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span
          className={cn(
            "text-[11.5px] px-2 py-0.5 rounded-cb-1 border",
            connected
              ? "bg-surface-2 border-line text-t2"
              : "bg-surface-3 border-line text-t4",
          )}
        >
          {connected ? "Connected" : "Not connected"}
        </span>
        {!connected && connectUrl && (
          <a
            href={connectUrl}
            className={cn(
              "flex items-center gap-1 text-[12px] text-t2 px-2.5 py-1 rounded-cb-1",
              "bg-surface-1 border border-line",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            )}
          >
            Connect
            <ExternalLink size={11} aria-hidden="true" />
          </a>
        )}
      </div>
    </div>
    {children && <div className="mt-3 pl-5">{children}</div>}
  </div>
);

// ── Notion section ────────────────────────────────────────────────────────────

const NotionSection: FC = () => {
  const { data, isFetching, isError } = useNotionIntegrationsQuery();
  const connectMutation    = useNotionConnectMutation();
  const disconnectMutation = useNotionDisconnectMutation();

  const [tokenInput, setTokenInput] = useState("");
  const [showTokenForm, setShowTokenForm] = useState(false);
  const [savedToken, setSavedToken] = useState(false);

  const handleConnect = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!tokenInput.trim()) return;
      connectMutation.mutate(
        { access_token: tokenInput.trim() },
        {
          onSuccess: () => {
            setTokenInput("");
            setShowTokenForm(false);
            setSavedToken(true);
            setTimeout(() => setSavedToken(false), 3000);
          },
        },
      );
    },
    [connectMutation, tokenInput],
  );

  if (isFetching && !data) return null;
  if (isError || !data) return null;

  const workspaces: NotionIntegration[] = data.integrations;

  return (
    <div className="py-4 border-b border-line last:border-b-0">
      <div className="flex items-center gap-3">
        <StatusDot connected={workspaces.length > 0} />
        <div className="flex-1 min-w-0">
          <p className="text-[13px] font-[500] text-t1">Notion</p>
          <p className="text-[12px] text-t3 mt-0.5">
            {workspaces.length > 0
              ? `${workspaces.length} workspace${workspaces.length === 1 ? "" : "s"} connected`
              : "No Notion workspaces connected"}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {data.oauth_configured ? (
            <a
              href="/oauth/notion/callback"
              className={cn(
                "flex items-center gap-1 text-[12px] text-t2 px-2.5 py-1 rounded-cb-1",
                "bg-surface-1 border border-line",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              )}
            >
              <Plus size={12} aria-hidden="true" />
              Connect workspace
              <ExternalLink size={11} aria-hidden="true" />
            </a>
          ) : (
            <button
              type="button"
              onClick={() => setShowTokenForm((v) => !v)}
              className={cn(
                "flex items-center gap-1 text-[12px] text-t2 px-2.5 py-1 rounded-cb-1",
                "bg-surface-1 border border-line",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              )}
            >
              <Plus size={12} aria-hidden="true" />
              {showTokenForm ? "Cancel" : "Connect via token"}
            </button>
          )}
        </div>
      </div>

      {/* Manual token form (self-hosted / no OAuth) */}
      {showTokenForm && !data.oauth_configured && (
        <form onSubmit={handleConnect} className="mt-3 pl-5 flex flex-col gap-2">
          <p className="text-[12px] text-t3">
            Create an internal integration at{" "}
            <a
              href="https://notion.so/my-integrations"
              target="_blank"
              rel="noopener noreferrer"
              className="underline"
            >
              notion.so/my-integrations
            </a>{" "}
            and paste the integration token here.
          </p>
          <div className="flex flex-col sm:flex-row gap-2">
            <input
              type="password"
              value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              placeholder="secret_…"
              required
              disabled={connectMutation.isPending}
              className={cn(
                "flex-1 px-3 py-2 rounded-cb-1 text-[13px] font-mono",
                "bg-surface-3 border border-line text-t1 placeholder:text-t4",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "disabled:opacity-40 transition-colors duration-[--cb-dur]",
              )}
              aria-label="Notion integration token"
            />
            <button
              type="submit"
              disabled={connectMutation.isPending || !tokenInput.trim()}
              className={cn(
                "shrink-0 flex items-center gap-1.5 px-3 py-2 rounded-cb-1",
                "text-[12.5px] font-[500]",
                savedToken
                  ? "bg-surface-1 border border-line text-t4"
                  : "bg-[--primary] text-[--primary-foreground]",
                "disabled:opacity-60 disabled:cursor-not-allowed",
                "transition-colors duration-[--cb-dur]",
              )}
            >
              {connectMutation.isPending ? "Connecting…" : "Connect"}
            </button>
          </div>
          <FieldError error={connectMutation.error} />
        </form>
      )}

      {savedToken && (
        <p className="mt-2 pl-5 text-[12px] text-t3 flex items-center gap-1">
          <Check size={12} strokeWidth={2.5} />
          Connected successfully.
        </p>
      )}

      {/* Connected workspaces */}
      {workspaces.length > 0 && (
        <div className="mt-3 pl-5 flex flex-col gap-2">
          {workspaces.map((ws) => (
            <div
              key={ws.id}
              className="flex items-center justify-between gap-3 py-2 border-b border-line last:border-b-0"
            >
              <div>
                <p className="text-[12.5px] text-t1 font-[500]">
                  {ws.notion_workspace_name ?? "Unnamed workspace"}
                </p>
                {ws.notion_workspace_id && (
                  <p className="text-[11px] text-t4 font-mono">
                    {ws.notion_workspace_id}
                  </p>
                )}
              </div>
              <button
                type="button"
                aria-label={`Disconnect ${ws.notion_workspace_name ?? "workspace"}`}
                onClick={() => disconnectMutation.mutate(ws.id)}
                disabled={disconnectMutation.isPending}
                className={cn(
                  "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-3",
                  "transition-colors duration-[--cb-dur] disabled:opacity-40",
                )}
              >
                <X size={13} strokeWidth={2} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

// ── Calendars section ─────────────────────────────────────────────────────────

const CalendarsSection: FC = () => {
  const { data, isFetching, isError } = useCalendarsIntegrationsQuery();

  if (isFetching && !data) return null;
  if (isError || !data || data.accounts.length === 0) return null;

  return (
    <div className="py-4 border-b border-line last:border-b-0">
      <div className="flex items-center gap-3 mb-3">
        <StatusDot connected={data.accounts.length > 0} />
        <div className="flex-1">
          <p className="text-[13px] font-[500] text-t1">Calendars</p>
          <p className="text-[12px] text-t3 mt-0.5">
            {data.accounts.length} account{data.accounts.length === 1 ? "" : "s"} connected
          </p>
        </div>
      </div>
      <div className="pl-5 flex flex-col gap-3">
        {data.accounts.map((account: CalendarAccountData) => (
          <div key={account.id} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <Calendar size={12} className="text-t4 shrink-0" />
              <span className="text-[12.5px] text-t1 font-[500] capitalize">
                {account.provider}
              </span>
              {account.email && (
                <span className="text-[12px] text-t3">{account.email}</span>
              )}
              <span
                className={cn(
                  "ml-auto text-[11px] px-1.5 py-0.5 rounded-cb-1 border",
                  account.active
                    ? "bg-surface-2 border-line text-t2"
                    : "bg-surface-3 border-line text-t4",
                )}
              >
                {account.active ? "Active" : "Inactive"}
              </span>
            </div>
            {account.calendars.length > 0 && (
              <div className="pl-5 flex flex-col">
                {account.calendars.map((cal) => (
                  <div
                    key={cal.id}
                    className="flex items-center gap-2 py-1.5 text-[12px] text-t3"
                  >
                    {cal.color ? (
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0 border border-line"
                        style={{ backgroundColor: cal.color }}
                        aria-hidden="true"
                      />
                    ) : (
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0 bg-surface-3 border border-line"
                        aria-hidden="true"
                      />
                    )}
                    <span className="flex-1 truncate">{cal.name}</span>
                    {cal.primary && (
                      <span className="text-[10.5px] text-t4">Primary</span>
                    )}
                    <span
                      className={cn(
                        "text-[10.5px]",
                        cal.syncing ? "text-t2" : "text-t4",
                      )}
                    >
                      {cal.syncing ? "Syncing" : "Off"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

// ── ConnectionForm ────────────────────────────────────────────────────────────

const AUTH_TYPE_OPTIONS = [
  { value: "none",       label: "None" },
  { value: "bearer",     label: "Bearer token" },
  { value: "basic",      label: "Basic auth" },
  { value: "header",     label: "Custom header" },
];

interface ConnectionFormProps {
  initial?: Partial<ConnectionData>;
  onSave: (data: {
    name: string;
    base_url: string;
    auth_type: string;
    auth_header_name?: string;
    auth_username?: string;
    auth_secret?: string;
  }) => void;
  onCancel: () => void;
  isPending: boolean;
  error: Error | null;
}

const ConnectionForm: FC<ConnectionFormProps> = ({
  initial,
  onSave,
  onCancel,
  isPending,
  error,
}) => {
  const [name,           setName]           = useState(initial?.name           ?? "");
  const [baseUrl,        setBaseUrl]        = useState(initial?.base_url        ?? "");
  const [authType,       setAuthType]       = useState(initial?.auth_type       ?? "none");
  const [authHeaderName, setAuthHeaderName] = useState(initial?.auth_header_name ?? "");
  const [authUsername,   setAuthUsername]   = useState(initial?.auth_username   ?? "");
  const [authSecret,     setAuthSecret]     = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      name,
      base_url: baseUrl,
      auth_type: authType,
      ...(authHeaderName ? { auth_header_name: authHeaderName } : {}),
      ...(authUsername   ? { auth_username:    authUsername   } : {}),
      ...(authSecret     ? { auth_secret:      authSecret     } : {}),
    });
  };

  const inputClass = cn(
    "w-full px-3 py-2 rounded-cb-1 text-[13px]",
    "bg-surface-3 border border-line text-t1 placeholder:text-t4",
    "focus:outline-none focus:ring-2 focus:ring-[--ring]",
    "disabled:opacity-40 transition-colors duration-[--cb-dur]",
  );

  const formId = initial?.id != null ? `conn-${initial.id}` : "conn-new";

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div className="flex flex-col gap-1">
          <label htmlFor={`${formId}-name`} className="text-[12px] font-[500] text-t2">Name</label>
          <input
            id={`${formId}-name`}
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="My API"
            required
            disabled={isPending}
            className={inputClass}
          />
        </div>
        <div className="flex flex-col gap-1">
          <label htmlFor={`${formId}-url`} className="text-[12px] font-[500] text-t2">Base URL</label>
          <input
            id={`${formId}-url`}
            type="url"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.example.com"
            required
            disabled={isPending}
            className={inputClass}
          />
        </div>
      </div>

      <div className="flex flex-col gap-1">
        <label htmlFor={`${formId}-auth`} className="text-[12px] font-[500] text-t2">Auth type</label>
        <select
          id={`${formId}-auth`}
          value={authType}
          onChange={(e) => setAuthType(e.target.value)}
          disabled={isPending}
          className={inputClass}
        >
          {AUTH_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>

      {(authType === "header" || authType === "bearer") && (
        <div className="flex flex-col gap-1">
          <label htmlFor={`${formId}-hdr`} className="text-[12px] font-[500] text-t2">Header name</label>
          <input
            id={`${formId}-hdr`}
            type="text"
            value={authHeaderName}
            onChange={(e) => setAuthHeaderName(e.target.value)}
            placeholder={authType === "bearer" ? "Authorization" : "X-Api-Key"}
            disabled={isPending}
            className={inputClass}
          />
        </div>
      )}

      {authType === "basic" && (
        <div className="flex flex-col gap-1">
          <label htmlFor={`${formId}-user`} className="text-[12px] font-[500] text-t2">Username</label>
          <input
            id={`${formId}-user`}
            type="text"
            value={authUsername}
            onChange={(e) => setAuthUsername(e.target.value)}
            placeholder="api_user"
            disabled={isPending}
            className={inputClass}
          />
        </div>
      )}

      {authType !== "none" && (
        <div className="flex flex-col gap-1">
          <label htmlFor={`${formId}-secret`} className="text-[12px] font-[500] text-t2">
            Secret / token
            {initial?.has_secret && (
              <span className="ml-1 text-t4 font-normal">(leave blank to keep existing)</span>
            )}
          </label>
          <input
            id={`${formId}-secret`}
            type="password"
            value={authSecret}
            onChange={(e) => setAuthSecret(e.target.value)}
            placeholder={initial?.has_secret ? "••••••••" : "Enter secret"}
            disabled={isPending}
            className={inputClass}
          />
        </div>
      )}

      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={isPending || !name || !baseUrl}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "text-[12.5px] font-[500] bg-[--primary] text-[--primary-foreground]",
            "disabled:opacity-60 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          {isPending ? "Saving…" : "Save connection"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          disabled={isPending}
          className={cn(
            "px-3 py-1.5 rounded-cb-1 text-[12.5px] text-t3",
            "border border-line hover:bg-surface-2",
            "transition-colors duration-[--cb-dur] disabled:opacity-40",
          )}
        >
          Cancel
        </button>
        <FieldError error={error} />
      </div>
    </form>
  );
};

// ── ConnectionsSection ────────────────────────────────────────────────────────

const ConnectionsSection: FC = () => {
  const { data, isFetching, isError } = useConnectionsQuery();
  const createMutation = useCreateConnectionMutation();
  const updateMutation = useUpdateConnectionMutation();
  const deleteMutation = useDeleteConnectionMutation();

  const [showCreate, setShowCreate]  = useState(false);
  const [editingId,  setEditingId]   = useState<number | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<number | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const connections: ConnectionData[] = data?.data ?? [];

  const handleCreate = useCallback(
    (params: Parameters<typeof createMutation.mutate>[0]) => {
      createMutation.mutate(params, {
        onSuccess: () => setShowCreate(false),
      });
    },
    [createMutation],
  );

  const handleUpdate = useCallback(
    (id: number, params: Omit<Parameters<typeof updateMutation.mutate>[0], "id">) => {
      updateMutation.mutate(
        { id, ...params },
        { onSuccess: () => setEditingId(null) },
      );
    },
    [updateMutation],
  );

  const handleDeleteClick = (id: number) => {
    if (deleteConfirm === id) {
      // Second click = confirmed
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
      setDeleteConfirm(null);
      deleteMutation.mutate(id);
    } else {
      setDeleteConfirm(id);
      deleteTimerRef.current = setTimeout(() => setDeleteConfirm(null), 4000);
    }
  };

  return (
    <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
      <div className="px-4 py-3 sm:px-5 border-b border-line flex items-center gap-3">
        <div className="flex-1">
          <h2 className="text-[14px] font-[600] text-t1 flex items-center gap-2">
            <Plug size={14} className="text-t4" />
            Connections
          </h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Custom HTTP endpoints used by Scout and workflow actions.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setShowCreate((v) => !v)}
          className={cn(
            "flex items-center gap-1 text-[12px] text-t2 px-2.5 py-1 rounded-cb-1",
            "bg-surface-1 border border-line hover:bg-surface-2",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          <Plus size={12} />
          Add
        </button>
      </div>

      {/* Create form */}
      {showCreate && (
        <div className="px-4 py-4 sm:px-5 border-b border-line bg-surface-2">
          <ConnectionForm
            onSave={handleCreate}
            onCancel={() => setShowCreate(false)}
            isPending={createMutation.isPending}
            error={createMutation.error}
          />
        </div>
      )}

      {/* List */}
      <div className="px-4 sm:px-5">
        {isFetching && !data ? (
          <p className="py-4 text-[12.5px] text-t4">Loading…</p>
        ) : isError ? (
          <p className="py-4 text-[12.5px] text-danger">
            Could not load connections.
          </p>
        ) : connections.length === 0 ? (
          <p className="py-6 text-center text-[12.5px] text-t4">
            No connections yet. Add one to use in workflows.
          </p>
        ) : (
          connections.map((conn) => (
            <div key={conn.id} className="py-4 border-b border-line last:border-b-0">
              {editingId === conn.id ? (
                <ConnectionForm
                  initial={conn}
                  onSave={(p) => handleUpdate(conn.id, p)}
                  onCancel={() => setEditingId(null)}
                  isPending={updateMutation.isPending}
                  error={updateMutation.error}
                />
              ) : (
                <div className="flex items-start gap-3">
                  <div className="flex-1 min-w-0">
                    <p className="text-[13px] font-[500] text-t1">{conn.name}</p>
                    <p className="text-[12px] text-t3 truncate">{conn.base_url}</p>
                    <p className="text-[11.5px] text-t4 mt-0.5 capitalize">
                      Auth: {AUTH_TYPE_OPTIONS.find((o) => o.value === conn.auth_type)?.label ?? conn.auth_type}
                    </p>
                  </div>
                  <div className="flex items-center gap-1.5 shrink-0">
                    <button
                      type="button"
                      aria-label={`Edit ${conn.name}`}
                      onClick={() => setEditingId(conn.id)}
                      className={cn(
                        "p-1.5 rounded-cb-1 text-t4 hover:text-t1 hover:bg-surface-3",
                        "transition-colors duration-[--cb-dur]",
                      )}
                    >
                      <Pencil size={13} strokeWidth={2} />
                    </button>
                    <button
                      type="button"
                      aria-label={
                        deleteConfirm === conn.id
                          ? "Click again to confirm deletion"
                          : `Delete ${conn.name}`
                      }
                      onClick={() => handleDeleteClick(conn.id)}
                      disabled={deleteMutation.isPending}
                      className={cn(
                        "p-1.5 rounded-cb-1 transition-colors duration-[--cb-dur] disabled:opacity-40",
                        deleteConfirm === conn.id
                          ? "text-danger bg-surface-3"
                          : "text-t4 hover:text-danger hover:bg-surface-3",
                      )}
                    >
                      {deleteConfirm === conn.id ? (
                        <Trash2 size={13} strokeWidth={2} />
                      ) : (
                        <Trash2 size={13} strokeWidth={2} />
                      )}
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </section>
  );
};

// ── IntegrationsPage ──────────────────────────────────────────────────────────

export const IntegrationsPage: FC = () => {
  const { data, isFetching, isError, error, refetch } =
    useIntegrationsOverviewQuery();

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading integrations"
      >
        <div className="h-6 w-44 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="h-16 bg-surface-1 rounded-cb-1" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.page.error"
        className="flex flex-col items-center justify-center gap-3 py-16 text-center"
      >
        <Visor state="asleep" size={28} label="Error loading integrations" />
        <p className="text-[13px] text-t2">
          {error instanceof Error ? error.message : "Something went wrong"}
        </p>
        <button
          type="button"
          onClick={() => refetch()}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12.5px] text-t2",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
          data-testid="settings.page.retry"
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div data-testid="settings.page.root" className="max-w-2xl">
      <h1 className="text-[17px] font-[650] text-t1 mb-6">Integrations</h1>

      {/* Connected services overview */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
        <div className="px-4 py-3 sm:px-5 border-b border-line">
          <h2 className="text-[14px] font-[600] text-t1">Connected services</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Services connected to this workspace for document storage and automation.
          </p>
        </div>
        <div className="px-4 sm:px-5">
          {/* Google Drive */}
          <StatusCard
            name="Google Drive"
            connected={data.google_drive.connected}
            detail={
              data.google_drive.connected
                ? `${data.google_drive.account_count ?? 0} account${(data.google_drive.account_count ?? 0) === 1 ? "" : "s"} connected`
                : "Connect to automatically archive documents."
            }
            connectUrl="/email_accounts/new?provider=google"
          />

          {/* Notion — detailed section */}
          <NotionSection />

          {/* Zoho Drive */}
          <StatusCard
            name="Zoho Drive"
            connected={data.zoho_drive.connected}
            detail={
              data.zoho_drive.connected
                ? `${data.zoho_drive.account_count ?? 0} account${(data.zoho_drive.account_count ?? 0) === 1 ? "" : "s"} connected`
                : "Connects automatically when a Zoho mailbox is linked."
            }
          />

          {/* Calendars — detailed section */}
          {data.calendars.connected ? (
            <CalendarsSection />
          ) : (
            <StatusCard
              name="Calendar"
              connected={false}
              detail="Connects automatically when a Google or Zoho mailbox is linked."
            />
          )}
        </div>
      </section>

      {/* Connections CRUD */}
      <ConnectionsSection />
    </div>
  );
};
