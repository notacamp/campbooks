/**
 * FilteringSection — Inbox Settings → Filtering tab.
 *
 * Shows the inbox filter strategy selector and the blocked / starred / allowed
 * contact lists. Contacts can be unblocked / removed from the starred/allowed lists.
 */
import { type FC, useCallback } from "react";
import { RotateCcw, X } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useFilteringQuery,
  useUpdateFilteringMutation,
  useSetSenderFilterMutation,
} from "~/modules/settings/api/use-inbox-filtering";
import type { FilterContact } from "./types";
import { cn } from "~/lib/utils";

// ── Strategy options ──────────────────────────────────────────────────────────

const STRATEGY_OPTIONS = [
  { value: "all",          label: "All — show every email" },
  { value: "known",        label: "Known — only from contacts" },
  { value: "allowed",      label: "Allowed — only from explicitly allowed contacts" },
];

// ── Contact chip ──────────────────────────────────────────────────────────────

interface ContactChipProps {
  contact: FilterContact;
  removeLabel: string;
  onRemove: (id: number) => void;
  disabled: boolean;
}

const ContactChip: FC<ContactChipProps> = ({ contact, removeLabel, onRemove, disabled }) => (
  <div
    className={cn(
      "flex items-center gap-2 pl-3 pr-2 py-1.5 rounded-full",
      "bg-surface-2 border border-line",
    )}
  >
    <span className="text-[12.5px] text-t2 max-w-[180px] truncate">
      {contact.name ? `${contact.name} (${contact.email})` : contact.email}
    </span>
    <button
      type="button"
      onClick={() => onRemove(contact.id)}
      disabled={disabled}
      aria-label={`${removeLabel} ${contact.email}`}
      className={cn(
        "flex items-center justify-center w-4 h-4 rounded-full",
        "text-t4 hover:text-danger hover:bg-surface-3",
        "transition-colors duration-[--cb-dur]",
        "disabled:opacity-40 disabled:cursor-not-allowed",
      )}
    >
      <X size={10} strokeWidth={2.5} aria-hidden="true" />
    </button>
  </div>
);

// ── Contact group ─────────────────────────────────────────────────────────────

interface ContactGroupProps {
  title: string;
  description: string;
  contacts: FilterContact[];
  removeState: "unblock" | "neutral" | "unstar";
  removeLabel: string;
  onRemove: (id: number, state: "unblock" | "neutral" | "unstar") => void;
  disabled: boolean;
}

const ContactGroup: FC<ContactGroupProps> = ({
  title,
  description,
  contacts,
  removeState,
  removeLabel,
  onRemove,
  disabled,
}) => (
  <section className="bg-surface-1 cb-raised rounded-cb-2 mb-4">
    <div className="px-4 py-3 sm:px-5 border-b border-line">
      <h3 className="text-[13px] font-[600] text-t1">{title}</h3>
      <p className="text-[12px] text-t3 mt-0.5">{description}</p>
    </div>
    <div className="px-4 py-3 sm:px-5">
      {contacts.length === 0 ? (
        <p className="text-[12.5px] text-t4">None</p>
      ) : (
        <div className="flex flex-wrap gap-2">
          {contacts.map((c) => (
            <ContactChip
              key={c.id}
              contact={c}
              removeLabel={removeLabel}
              onRemove={(id) => onRemove(id, removeState)}
              disabled={disabled}
            />
          ))}
        </div>
      )}
    </div>
  </section>
);

// ── FilteringSection ──────────────────────────────────────────────────────────

export const FilteringSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useFilteringQuery();
  const updateMutation = useUpdateFilteringMutation();
  const setSenderMutation = useSetSenderFilterMutation();

  const handleStrategyChange = useCallback((strategy: string): void => {
    updateMutation.mutate({ inbox_filter_strategy: strategy });
  }, [updateMutation]);

  const handleRemove = useCallback(
    (id: number, state: "unblock" | "neutral" | "unstar"): void => {
      setSenderMutation.mutate({ contact_id: id, state });
    },
    [setSenderMutation],
  );

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.filtering.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading filtering settings"
      >
        <div className="h-16 bg-surface-1 rounded-cb-2" />
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-20 bg-surface-1 rounded-cb-2" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.inbox.filtering.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading filtering settings" />
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
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  const mutationPending = updateMutation.isPending || setSenderMutation.isPending;

  return (
    <div data-testid="settings.inbox.filtering.root">
      {/* Strategy */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 mb-4">
        <div className="px-4 py-3 sm:px-5 border-b border-line">
          <h3 className="text-[13px] font-[600] text-t1">Inbox filter strategy</h3>
          <p className="text-[12px] text-t3 mt-0.5">
            Controls which emails appear in your inbox by default.
          </p>
        </div>
        <div className="px-4 py-4 sm:px-5 flex flex-col gap-3">
          {STRATEGY_OPTIONS.map((opt) => (
            <label key={opt.value} className="flex items-start gap-3 cursor-pointer">
              <input
                type="radio"
                name="inbox_filter_strategy"
                value={opt.value}
                checked={data.strategy === opt.value}
                onChange={() => handleStrategyChange(opt.value)}
                disabled={mutationPending}
                className="mt-0.5 accent-[--ring]"
              />
              <span className="text-[13px] text-t2">{opt.label}</span>
            </label>
          ))}
          {updateMutation.error && (
            <p className="text-[12px] text-danger mt-1" role="alert">
              {updateMutation.error instanceof ApiError
                ? updateMutation.error.message
                : "Failed to save"}
            </p>
          )}
        </div>
      </section>

      {/* Starred contacts */}
      <ContactGroup
        title="Starred senders"
        description="Emails from these contacts are always shown at the top."
        contacts={data.starred}
        removeState="unstar"
        removeLabel="Unstar"
        onRemove={handleRemove}
        disabled={mutationPending}
      />

      {/* Allowed contacts */}
      <ContactGroup
        title="Allowed senders"
        description="Explicitly allowed when using the 'allowed' filter strategy."
        contacts={data.allowed}
        removeState="neutral"
        removeLabel="Remove allow"
        onRemove={handleRemove}
        disabled={mutationPending}
      />

      {/* Blocked contacts */}
      <ContactGroup
        title="Blocked senders"
        description="Emails from blocked senders are hidden from the inbox."
        contacts={data.blocked}
        removeState="unblock"
        removeLabel="Unblock"
        onRemove={handleRemove}
        disabled={mutationPending}
      />

      {setSenderMutation.error && (
        <p className="text-[12px] text-danger mt-1" role="alert">
          {setSenderMutation.error instanceof ApiError
            ? setSenderMutation.error.message
            : "Failed to update sender"}
        </p>
      )}
    </div>
  );
};
