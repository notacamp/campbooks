/**
 * WorkspacePage — Settings → General at /settings/workspace.
 *
 * Fetches GET /api/app/settings/workspace and renders editable fields:
 *   - Workspace name
 *   - AI persona / workspace context (textarea)
 *   - Company NIF
 *
 * All fields save together via PATCH /api/app/settings/workspace.
 * Admin-only mutations; non-admins see the fields read-only.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useWorkspaceQuery,
  useWorkspaceMutation,
} from "~/modules/settings/api";
import { useMe } from "~/lib/api";
import { cn } from "~/lib/utils";

// ── Shared form helpers (local, light-weight) ─────────────────────────────────

const FieldRow: FC<{
  label: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ label, hint, children }) => (
  <div className="flex flex-col sm:flex-row sm:items-start gap-2 sm:gap-6 py-4 border-b border-line last:border-b-0">
    <div className="sm:w-[180px] shrink-0">
      <span className="text-[13px] font-[500] text-t1">{label}</span>
      {hint && <p className="text-[12px] text-t4 mt-0.5 leading-snug">{hint}</p>}
    </div>
    <div className="flex-1 min-w-0">{children}</div>
  </div>
);

const FieldError: FC<{ error: Error | null }> = ({ error }) => {
  if (!error) return null;
  const msg = error instanceof ApiError ? error.message : "Something went wrong";
  return (
    <p className="text-[12px] text-danger mt-1.5" role="alert">
      {msg}
    </p>
  );
};

// ── WorkspacePage ─────────────────────────────────────────────────────────────

export const WorkspacePage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useWorkspaceQuery();
  const { data: me } = useMe();
  const mutation = useWorkspaceMutation();

  const isAdmin = me?.user?.role === "admin";

  // Controlled state — local drafts
  const [name,    setName]    = useState<string | null>(null);
  const [context, setContext] = useState<string | null>(null);
  const [nif,     setNif]     = useState<string | null>(null);
  const [saved,   setSaved]   = useState(false);

  const effectiveName    = name    ?? data?.name             ?? "";
  const effectiveContext = context ?? data?.workspace_context ?? "";
  const effectiveNif     = nif     ?? data?.company_nif      ?? "";

  const handleSave = useCallback((): void => {
    mutation.mutate(
      {
        name:              effectiveName,
        workspace_context: effectiveContext,
        company_nif:       effectiveNif,
      },
      {
        onSuccess: () => {
          setName(null);
          setContext(null);
          setNif(null);
          setSaved(true);
          setTimeout(() => setSaved(false), 2000);
        },
      },
    );
  }, [mutation, effectiveName, effectiveContext, effectiveNif]);

  // ── Loading / error ────────────────────────────────────────────────────────

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading workspace settings"
      >
        <div className="h-6 w-48 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-14 bg-surface-1 rounded-cb-1" />
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
        <Visor state="asleep" size={28} label="Error loading workspace settings" />
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

  // ── Content ────────────────────────────────────────────────────────────────

  return (
    <div
      data-testid="settings.page.root"
      className="max-w-2xl"
    >
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-[17px] font-[650] text-t1">General</h1>
        {!isAdmin && (
          <span className="text-[12px] text-t4 bg-surface-1 border border-line px-2 py-1 rounded-cb-1">
            View only — admins only
          </span>
        )}
      </div>

      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        {/* Workspace name */}
        <FieldRow label="Name" hint="Displayed in the sidebar and emails.">
          <input
            type="text"
            value={effectiveName}
            onChange={(e) => setName(e.target.value)}
            disabled={!isAdmin}
            placeholder="Your workspace name"
            className={cn(
              "w-full px-3 py-2 rounded-cb-1 text-[13px]",
              "bg-surface-3 border border-line",
              "text-t1 placeholder:text-t4",
              "focus:outline-none focus:ring-2 focus:ring-[--ring]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
              "transition-colors duration-[--cb-dur]",
            )}
            aria-label="Workspace name"
          />
        </FieldRow>

        {/* AI persona / workspace context */}
        <FieldRow
          label="Scout's context"
          hint="Tell Scout what this workspace is about. Used when drafting emails and analysing documents."
        >
          <textarea
            value={effectiveContext}
            onChange={(e) => setContext(e.target.value)}
            disabled={!isAdmin}
            placeholder="E.g. We are a small property agency in Lisbon. Our main contacts are landlords and tenants."
            rows={4}
            className={cn(
              "w-full px-3 py-2 rounded-cb-1 text-[13px] resize-y",
              "bg-surface-3 border border-line",
              "text-t1 placeholder:text-t4",
              "focus:outline-none focus:ring-2 focus:ring-[--ring]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
              "transition-colors duration-[--cb-dur]",
            )}
            aria-label="Scout's workspace context"
          />
        </FieldRow>

        {/* Company NIF */}
        <FieldRow
          label="Company NIF"
          hint="Your tax identification number. Appears on invoice templates."
        >
          <input
            type="text"
            value={effectiveNif}
            onChange={(e) => setNif(e.target.value)}
            disabled={!isAdmin}
            placeholder="PT123456789"
            className={cn(
              "w-full max-w-xs px-3 py-2 rounded-cb-1 text-[13px]",
              "bg-surface-3 border border-line",
              "text-t1 placeholder:text-t4",
              "focus:outline-none focus:ring-2 focus:ring-[--ring]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
              "transition-colors duration-[--cb-dur]",
            )}
            aria-label="Company NIF"
          />
        </FieldRow>

        {/* Read-only info */}
        <FieldRow label="Plan">
          <span className="text-[13px] text-t2 capitalize">{data.plan}</span>
        </FieldRow>

        <FieldRow label="Workspace ID">
          <code className="text-[12px] text-t3 bg-surface-3 px-1.5 py-0.5 rounded-cb-1 font-mono">
            {data.slug}
          </code>
        </FieldRow>
      </section>

      {/* Save */}
      {isAdmin && (
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleSave}
            disabled={mutation.isPending || saved}
            className={cn(
              "flex items-center gap-1.5 px-4 py-2 rounded-cb-1 text-[13px] font-[500]",
              saved
                ? "bg-surface-1 border border-line text-t4"
                : "bg-[--primary] text-[--primary-foreground]",
              "disabled:opacity-60 disabled:cursor-not-allowed",
              "transition-colors duration-[--cb-dur]",
            )}
          >
            {saved ? (
              <>
                <Check size={13} strokeWidth={2.5} aria-hidden="true" />
                Saved
              </>
            ) : mutation.isPending ? (
              "Saving…"
            ) : (
              "Save changes"
            )}
          </button>
          <FieldError error={mutation.error} />
        </div>
      )}
    </div>
  );
};
