/**
 * PrivacyPage — Settings → Data & privacy at /settings/privacy.
 *
 * Fetches GET /api/app/settings/data_privacy and renders:
 *   - AI processing enabled (toggle)
 *   - Scout thread posts (toggle — can Scout post to threads?)
 *   - Required data region (select: auto | eu | us)
 *   - Email retention months (number input)
 *   - Connected integrations overview (read-only status)
 *
 * Saves immediately on toggle change (no explicit "Save" needed for toggles),
 * with a "Save" button for the text fields.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import { usePrivacyQuery, usePrivacyMutation } from "~/modules/settings/api";
import { cn } from "~/lib/utils";

// ── Toggle ────────────────────────────────────────────────────────────────────

interface ToggleProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}

const Toggle: FC<ToggleProps> = ({ checked, onChange, label, disabled }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    disabled={disabled}
    onClick={() => onChange(!checked)}
    className={cn(
      "relative inline-flex items-center h-5 w-9 rounded-full shrink-0",
      "focus:outline-none focus:ring-2 focus:ring-[--ring] focus:ring-offset-2",
      "transition-colors duration-[--cb-dur]",
      checked ? "bg-accent-cb" : "bg-surface-3 border border-line",
      "disabled:opacity-40 disabled:cursor-not-allowed",
    )}
  >
    <span
      className={cn(
        "inline-block w-3.5 h-3.5 rounded-full bg-white shadow-sm",
        "transform transition-transform duration-[--cb-dur]",
        checked ? "translate-x-[18px]" : "translate-x-0.5",
      )}
    />
  </button>
);

interface ToggleRowProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}

const ToggleRow: FC<ToggleRowProps> = ({ label, description, checked, onChange, disabled }) => (
  <div className="flex items-start gap-4 py-4 border-b border-line last:border-b-0">
    <div className="flex-1 min-w-0">
      <p className="text-[13px] font-[500] text-t1">{label}</p>
      {description && (
        <p className="text-[12px] text-t3 mt-0.5 leading-snug">{description}</p>
      )}
    </div>
    <Toggle
      checked={checked}
      onChange={onChange}
      label={label}
      disabled={disabled}
    />
  </div>
);

// ── Data region options ───────────────────────────────────────────────────────

const REGION_OPTIONS = [
  { value: "",   label: "Automatic (follow Scout AI model)" },
  { value: "eu", label: "EU — European Union" },
  { value: "us", label: "US — United States" },
];

// ── Connected integration indicator ──────────────────────────────────────────

interface ConnectedBadgeProps {
  connected: boolean;
  name: string;
}

const ConnectedBadge: FC<ConnectedBadgeProps> = ({ connected, name }) => (
  <div className="flex items-center gap-2 py-2">
    <span
      className={cn(
        "inline-block w-2 h-2 rounded-full shrink-0",
        connected ? "bg-money-text" : "bg-t4",
      )}
      aria-hidden="true"
    />
    <span className="text-[12.5px] text-t2">{name}</span>
    <span className="text-[11.5px] text-t4 ml-auto">
      {connected ? "Connected" : "Not connected"}
    </span>
  </div>
);

// ── PrivacyPage ───────────────────────────────────────────────────────────────

export const PrivacyPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = usePrivacyQuery();
  const mutation = usePrivacyMutation();

  const [retentionMonths, setRetentionMonths] = useState<number | null>(null);
  const [dataRegion,      setDataRegion]      = useState<string | null>(null);
  const [saved,           setSaved]           = useState(false);

  // Immediate toggle save
  const handleToggle = useCallback(
    (field: "ai_processing_enabled" | "scout_thread_posts", value: boolean): void => {
      mutation.mutate({ [field]: value });
    },
    [mutation],
  );

  const handleSaveFields = useCallback((): void => {
    const params: Record<string, unknown> = {};
    if (retentionMonths !== null) params.email_retention_months = retentionMonths;
    if (dataRegion !== null) params.required_data_region = dataRegion;
    mutation.mutate(params, {
      onSuccess: () => {
        setRetentionMonths(null);
        setDataRegion(null);
        setSaved(true);
        setTimeout(() => setSaved(false), 2000);
      },
    });
  }, [mutation, retentionMonths, dataRegion]);

  // ── Loading / error ────────────────────────────────────────────────────────

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading privacy settings"
      >
        <div className="h-6 w-44 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2].map((i) => (
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
        <Visor state="asleep" size={28} label="Error loading privacy settings" />
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

  const effectiveRegion    = dataRegion      ?? data.required_data_region      ?? "";
  const effectiveRetention = retentionMonths ?? data.email_retention_months    ?? 12;

  // ── Content ────────────────────────────────────────────────────────────────

  return (
    <div
      data-testid="settings.page.root"
      className="max-w-2xl"
    >
      <h1 className="text-[17px] font-[650] text-t1 mb-6">Data &amp; privacy</h1>

      {/* AI processing */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <div className="px-4 py-3 sm:px-5">
          <h2 className="text-[14px] font-[600] text-t1">AI processing</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Controls whether Scout analyses your email content.
          </p>
        </div>
        <div className="px-4 sm:px-5">
          <ToggleRow
            label="AI email processing"
            description="Scout reads each new email to identify what needs your attention, classify documents, and draft replies."
            checked={data.ai_processing_enabled}
            onChange={(v) => handleToggle("ai_processing_enabled", v)}
            disabled={mutation.isPending}
          />
          {data.scout_thread_posts !== null && (
            <ToggleRow
              label="Scout thread posts"
              description="Scout can post a brief summary note into email threads it processed."
              checked={data.scout_thread_posts ?? false}
              onChange={(v) => handleToggle("scout_thread_posts", v)}
              disabled={mutation.isPending}
            />
          )}
        </div>
      </section>

      {/* Data region */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <div className="px-4 py-3 sm:px-5">
          <h2 className="text-[14px] font-[600] text-t1">Data residency</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Restrict which region AI calls and data storage may use.
          </p>
        </div>
        <div className="px-4 py-4 sm:px-5 flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="data-region"
              className="text-[13px] font-[500] text-t1"
            >
              Required region
            </label>
            <select
              id="data-region"
              value={effectiveRegion}
              onChange={(e) => setDataRegion(e.target.value)}
              className={cn(
                "w-full max-w-xs px-3 py-2 rounded-cb-1 text-[13px]",
                "bg-surface-3 border border-line text-t1",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
            >
              {REGION_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>

          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="retention-months"
              className="text-[13px] font-[500] text-t1"
            >
              Email retention (months)
            </label>
            <p className="text-[12px] text-t4">
              Campbooks keeps a local copy of emails for this many months, then purges them.
            </p>
            <input
              id="retention-months"
              type="number"
              min={1}
              max={120}
              value={effectiveRetention}
              onChange={(e) => setRetentionMonths(Math.max(1, Number(e.target.value)))}
              className={cn(
                "w-24 px-3 py-2 rounded-cb-1 text-[13px] tabular-nums",
                "bg-surface-3 border border-line text-t1",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label="Email retention months"
            />
          </div>

          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleSaveFields}
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
                "Save"
              )}
            </button>
            {mutation.error && (
              <p className="text-[12px] text-danger" role="alert">
                {mutation.error instanceof ApiError
                  ? mutation.error.message
                  : "Something went wrong"}
              </p>
            )}
          </div>
        </div>
      </section>

      {/* Connected integrations */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <div className="px-4 py-3 sm:px-5">
          <h2 className="text-[14px] font-[600] text-t1">Connected integrations</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Services that have data access via this workspace.
          </p>
        </div>
        <div className="px-4 py-3 sm:px-5 flex flex-col">
          <ConnectedBadge name="Google Drive"   connected={data.google_drive_connected} />
          <ConnectedBadge name="Notion"         connected={data.notion_connected} />
          <ConnectedBadge name="Zoho Drive"     connected={data.zoho_drive_connected} />
          <ConnectedBadge name="Calendar"       connected={data.calendar_connected} />
        </div>
      </section>
    </div>
  );
};
