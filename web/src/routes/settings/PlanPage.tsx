/**
 * PlanPage — Settings → Plan at /settings/plan.
 *
 * Fetches GET /api/app/settings/plan and renders a read-only view of:
 *   - Current plan name
 *   - Per-feature flags (workflows, digests, accounting, imap, …)
 *   - Per-resource limits (members, email accounts, API clients)
 *
 * Read-only for all workspace members.
 */
import { type FC } from "react";
import { RotateCcw } from "lucide-react";
import { Visor } from "~/lib/ui";
import { usePlanQuery } from "~/modules/settings/api";
import { cn } from "~/lib/utils";

// ── Feature labels ─────────────────────────────────────────────────────────────

const FEATURE_LABELS: Record<string, string> = {
  workflows:          "Workflows",
  email_board:        "Email board",
  document_templates: "Document templates",
  email_templates:    "Email templates",
  tasks:              "Asks (tasks)",
  digests:            "Email digests",
  accounting:         "Accounting / reconciliation",
  imap:               "IMAP mailboxes",
  microsoft:          "Microsoft 365",
};

const LIMIT_LABELS: Record<string, string> = {
  members:        "Team members",
  email_accounts: "Email accounts",
  api_clients:    "API clients",
};

// ── Shared field row ──────────────────────────────────────────────────────────

const FieldRow: FC<{
  label: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ label, hint, children }) => (
  <div className="flex flex-col sm:flex-row sm:items-start gap-2 sm:gap-6 py-4 border-b border-line last:border-b-0">
    <div className="sm:w-[200px] shrink-0">
      <span className="text-[13px] font-[500] text-t1">{label}</span>
      {hint && <p className="text-[12px] text-t4 mt-0.5 leading-snug">{hint}</p>}
    </div>
    <div className="flex-1 min-w-0">{children}</div>
  </div>
);

// ── PlanBadge ─────────────────────────────────────────────────────────────────

const PlanBadge: FC<{ plan: string }> = ({ plan }) => (
  <span
    className={cn(
      "inline-flex items-center px-2.5 py-1 rounded-cb-1 text-[12px] font-[600] capitalize",
      plan === "free"
        ? "bg-surface-3 text-t2 border border-line"
        : "bg-[--accent-cb] text-white",
    )}
  >
    {plan}
  </span>
);

// ── FeatureFlag ───────────────────────────────────────────────────────────────

const FeatureDot: FC<{ enabled: boolean; label: string }> = ({ enabled, label }) => (
  <div className="flex items-center gap-2 py-2 border-b border-line last:border-b-0">
    <span
      className={cn(
        "inline-block w-2 h-2 rounded-full shrink-0",
        enabled ? "bg-money-text" : "bg-t4",
      )}
      aria-hidden="true"
    />
    <span className="text-[13px] text-t1 flex-1">{label}</span>
    <span className="text-[12px] text-t4">
      {enabled ? "Included" : "Not included"}
    </span>
  </div>
);

// ── LimitRow ──────────────────────────────────────────────────────────────────

interface LimitBarProps {
  label: string;
  usage: number;
  limit: number | null;
  remaining: number | null;
}

const LimitBar: FC<LimitBarProps> = ({ label, usage, limit, remaining }) => {
  const pct = limit != null && limit > 0 ? Math.min((usage / limit) * 100, 100) : 0;
  const isUnlimited = limit == null;

  return (
    <div className="flex flex-col gap-1.5 py-3 border-b border-line last:border-b-0">
      <div className="flex items-center justify-between">
        <span className="text-[13px] font-[500] text-t1">{label}</span>
        <span className="text-[12px] text-t3">
          {isUnlimited ? (
            <span className="text-t2">Unlimited</span>
          ) : (
            <>
              <span className="text-t1 font-[600]">{usage}</span>
              <span className="text-t4"> / {limit}</span>
              {remaining != null && remaining <= 2 && (
                <span className="ml-2 text-danger">{remaining} remaining</span>
              )}
            </>
          )}
        </span>
      </div>
      {!isUnlimited && (
        <div
          className="h-1.5 rounded-full bg-surface-3 overflow-hidden"
          aria-hidden="true"
        >
          <div
            className={cn(
              "h-full rounded-full transition-all duration-300",
              pct >= 90 ? "bg-danger" : pct >= 70 ? "bg-[--accent-cb]" : "bg-money-text",
            )}
            style={{ width: `${pct}%` }}
          />
        </div>
      )}
    </div>
  );
};

// ── PlanPage ──────────────────────────────────────────────────────────────────

export const PlanPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = usePlanQuery();

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading plan settings"
      >
        <div className="h-6 w-32 bg-surface-1 rounded-cb-1" />
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
        <Visor state="asleep" size={28} label="Error loading plan" />
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

  const featureEntries = Object.entries(data.features);
  const limitEntries = Object.entries(data.limits);

  return (
    <div data-testid="settings.page.root" className="max-w-2xl">
      <h1 className="text-[17px] font-[650] text-t1 mb-6">Plan</h1>

      {/* Current plan */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <FieldRow label="Current plan">
          <div className="flex items-center gap-3">
            <PlanBadge plan={data.plan} />
            {data.self_hosted && (
              <span className="text-[11.5px] text-t4 bg-surface-3 border border-line px-2 py-0.5 rounded-cb-1">
                Self-hosted
              </span>
            )}
          </div>
        </FieldRow>
        {data.available_plans.length > 0 && (
          <FieldRow label="Available plans">
            <div className="flex flex-wrap gap-2">
              {data.available_plans.map((p) => (
                <span
                  key={p}
                  className={cn(
                    "text-[12px] px-2 py-0.5 rounded-cb-1 border capitalize",
                    p === data.plan
                      ? "bg-[--accent-cb] text-white border-transparent"
                      : "bg-surface-3 text-t3 border-line",
                  )}
                >
                  {p}
                </span>
              ))}
            </div>
          </FieldRow>
        )}
      </section>

      {/* Usage limits */}
      {limitEntries.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line">
            <h2 className="text-[14px] font-[600] text-t1">Usage</h2>
            <p className="text-[12px] text-t3 mt-0.5">
              How much of each resource this workspace is using.
            </p>
          </div>
          <div className="px-4 sm:px-5">
            {limitEntries.map(([key, limit]) => (
              <LimitBar
                key={key}
                label={LIMIT_LABELS[key] ?? key}
                usage={limit.usage}
                limit={limit.limit}
                remaining={limit.remaining}
              />
            ))}
          </div>
        </section>
      )}

      {/* Feature flags */}
      {featureEntries.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line">
            <h2 className="text-[14px] font-[600] text-t1">Features</h2>
            <p className="text-[12px] text-t3 mt-0.5">
              What is included in your current plan.
            </p>
          </div>
          <div className="px-4 sm:px-5">
            {featureEntries.map(([key, enabled]) => (
              <FeatureDot
                key={key}
                enabled={enabled}
                label={FEATURE_LABELS[key] ?? key}
              />
            ))}
          </div>
        </section>
      )}
    </div>
  );
};
