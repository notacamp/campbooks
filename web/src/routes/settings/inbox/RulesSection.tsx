/**
 * RulesSection — Inbox Settings → Rules tab.
 *
 * Inbox group rules (email filtering automations). Shows list of rules with
 * enable/disable toggle, run action, and delete. Create opens an inline form
 * with basic criteria (from, to, subject) and actions (archive, mark_read).
 */
import { type FC, useState, useCallback } from "react";
import { Plus, Play, ToggleLeft, ToggleRight, Trash2, RotateCcw, ChevronDown, ChevronUp } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useRulesQuery,
  useCreateRuleMutation,
  useDeleteRuleMutation,
  useToggleRuleMutation,
  useRunRuleMutation,
} from "~/modules/settings/api/use-inbox-rules";
import type { Rule } from "./types";
import { cn } from "~/lib/utils";

// ── Criteria badge ────────────────────────────────────────────────────────────

const CriteriaBadge: FC<{ rule: Rule }> = ({ rule }) => {
  const parts: string[] = [];
  const c = rule.criteria;
  if (c.from) parts.push(`from: ${c.from}`);
  if (c.to) parts.push(`to: ${c.to}`);
  if (c.subject) parts.push(`subject: ${c.subject}`);
  if (c.body) parts.push(`body: ${c.body}`);
  if (c.has_attachment) parts.push("has attachment");
  if (c.category?.length) parts.push(`category: ${c.category.join(", ")}`);

  return (
    <p className="text-[11.5px] text-t3 mt-0.5 truncate">
      {parts.length > 0 ? parts.join(" · ") : "No criteria"}
    </p>
  );
};

// ── Actions badge ─────────────────────────────────────────────────────────────

const ActionsBadge: FC<{ rule: Rule }> = ({ rule }) => {
  const actions: string[] = [];
  if (rule.archive) actions.push("Archive");
  if (rule.mark_read) actions.push("Mark read");
  if (rule.tag_ids.length) actions.push(`Tag (${rule.tag_ids.length})`);

  return (
    <p className="text-[11.5px] text-t4 mt-0.5">
      {actions.length > 0 ? `→ ${actions.join(", ")}` : "No actions"}
    </p>
  );
};

// ── Rule row ──────────────────────────────────────────────────────────────────

interface RuleRowProps {
  rule: Rule;
  onToggle: (id: number) => void;
  onDelete: (id: number) => void;
  onRun: (id: number) => void;
  isPending: boolean;
  runSuccess: boolean;
}

const RuleRow: FC<RuleRowProps> = ({ rule, onToggle, onDelete, onRun, isPending, runSuccess }) => (
  <div
    className={cn(
      "flex items-start gap-3 py-3 px-4 sm:px-5 border-b border-line last:border-b-0",
      !rule.enabled && "opacity-60",
    )}
  >
    <div className="flex-1 min-w-0">
      <p className="text-[13px] font-[500] text-t1">{rule.name}</p>
      <CriteriaBadge rule={rule} />
      <ActionsBadge rule={rule} />
      {rule.last_run_at && (
        <p className="text-[11px] text-t4 mt-1">
          Last run: {new Date(rule.last_run_at).toLocaleDateString()}
        </p>
      )}
    </div>

    <div className="flex items-center gap-1 shrink-0 pt-0.5">
      <button
        type="button"
        onClick={() => onRun(rule.id)}
        title="Run rule now"
        disabled={isPending}
        className={cn(
          "p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2",
          "transition-colors duration-[--cb-dur]",
          "disabled:opacity-40 disabled:cursor-not-allowed",
          runSuccess && "text-money-text",
        )}
        aria-label={`Run rule ${rule.name}`}
      >
        <Play size={13} strokeWidth={2} aria-hidden="true" />
      </button>

      <button
        type="button"
        onClick={() => onToggle(rule.id)}
        title={rule.enabled ? "Disable" : "Enable"}
        disabled={isPending}
        className={cn(
          "p-1.5 rounded-cb-1 hover:bg-surface-2",
          rule.enabled ? "text-accent-cb" : "text-t4",
          "transition-colors duration-[--cb-dur]",
          "disabled:opacity-40 disabled:cursor-not-allowed",
        )}
        aria-label={rule.enabled ? `Disable ${rule.name}` : `Enable ${rule.name}`}
      >
        {rule.enabled ? (
          <ToggleRight size={16} strokeWidth={1.5} aria-hidden="true" />
        ) : (
          <ToggleLeft size={16} strokeWidth={1.5} aria-hidden="true" />
        )}
      </button>

      <button
        type="button"
        onClick={() => onDelete(rule.id)}
        title="Delete"
        disabled={isPending}
        className={cn(
          "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-2",
          "transition-colors duration-[--cb-dur]",
          "disabled:opacity-40 disabled:cursor-not-allowed",
        )}
        aria-label={`Delete rule ${rule.name}`}
      >
        <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
      </button>
    </div>
  </div>
);

// ── Add rule form ─────────────────────────────────────────────────────────────

const AddRuleForm: FC<{ onDone: () => void }> = ({ onDone }) => {
  const [name, setName] = useState("");
  const [from, setFrom] = useState("");
  const [subject, setSubject] = useState("");
  const [archive, setArchive] = useState(false);
  const [markRead, setMarkRead] = useState(false);
  const [showMore, setShowMore] = useState(false);
  const createMutation = useCreateRuleMutation();

  const handleSubmit = useCallback((e: React.FormEvent): void => {
    e.preventDefault();
    if (!name.trim()) return;
    const criteria: Record<string, unknown> = {};
    if (from.trim()) criteria.from = from.trim();
    if (subject.trim()) criteria.subject = subject.trim();
    createMutation.mutate(
      {
        email_rule: {
          name: name.trim(),
          criteria,
          archive,
          mark_read: markRead,
        },
      },
      { onSuccess: () => onDone() },
    );
  }, [createMutation, name, from, subject, archive, markRead, onDone]);

  const inputClass = cn(
    "w-full px-3 py-2 rounded-cb-1 text-[13px]",
    "bg-surface-3 border border-line text-t1 placeholder:text-t4",
    "focus:outline-none focus:ring-2 focus:ring-[--ring]",
    "transition-colors duration-[--cb-dur]",
  );

  return (
    <form
      onSubmit={handleSubmit}
      className="p-4 sm:p-5 border-t border-line flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1.5">
        <label htmlFor="rule-form-name" className="text-[12.5px] font-[500] text-t2">Rule name</label>
        <input
          id="rule-form-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. Archive newsletters"
          className={inputClass}
        />
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor="rule-form-from" className="text-[12.5px] font-[500] text-t2">From (sender)</label>
        <input
          id="rule-form-from"
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          placeholder="e.g. newsletter@example.com"
          className={inputClass}
        />
      </div>

      <button
        type="button"
        onClick={() => setShowMore(!showMore)}
        className="flex items-center gap-1 text-[12px] text-t4 hover:text-t2 w-fit transition-colors duration-[--cb-dur]"
      >
        {showMore ? (
          <ChevronUp size={12} strokeWidth={2} aria-hidden="true" />
        ) : (
          <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
        )}
        {showMore ? "Fewer options" : "More options (subject, body)"}
      </button>

      {showMore && (
        <div className="flex flex-col gap-1.5">
          <label htmlFor="rule-form-subject" className="text-[12.5px] font-[500] text-t2">Subject contains</label>
          <input
            id="rule-form-subject"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            placeholder="e.g. unsubscribe"
            className={inputClass}
          />
        </div>
      )}

      <div className="flex flex-col gap-2">
        <p className="text-[12.5px] font-[500] text-t2">Actions</p>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={archive}
            onChange={(e) => setArchive(e.target.checked)}
            className="accent-[--ring]"
          />
          <span className="text-[13px] text-t2">Archive</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={markRead}
            onChange={(e) => setMarkRead(e.target.checked)}
            className="accent-[--ring]"
          />
          <span className="text-[13px] text-t2">Mark as read</span>
        </label>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={!name.trim() || createMutation.isPending}
          className={cn(
            "px-4 py-2 rounded-cb-1 text-[13px] font-[500]",
            "bg-[--primary] text-[--primary-foreground]",
            "disabled:opacity-50 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          {createMutation.isPending ? "Adding…" : "Add rule"}
        </button>
        <button
          type="button"
          onClick={onDone}
          className={cn(
            "px-4 py-2 rounded-cb-1 text-[13px]",
            "bg-surface-1 border border-line text-t2",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
        >
          Cancel
        </button>
        {createMutation.error && (
          <p className="text-[12px] text-danger" role="alert">
            {createMutation.error instanceof ApiError
              ? createMutation.error.message
              : "Error"}
          </p>
        )}
      </div>
    </form>
  );
};

// ── RulesSection ──────────────────────────────────────────────────────────────

export const RulesSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useRulesQuery();
  const toggleMutation = useToggleRuleMutation();
  const deleteMutation = useDeleteRuleMutation();
  const runMutation = useRunRuleMutation();
  const [addingRule, setAddingRule] = useState(false);
  const [lastRunId, setLastRunId] = useState<number | null>(null);

  const handleToggle = useCallback((id: number): void => {
    toggleMutation.mutate(id);
  }, [toggleMutation]);

  const handleDelete = useCallback((id: number): void => {
    deleteMutation.mutate(id);
  }, [deleteMutation]);

  const handleRun = useCallback((id: number): void => {
    runMutation.mutate(id, {
      onSuccess: (result) => {
        setLastRunId(result.run_id);
        setTimeout(() => setLastRunId(null), 3000);
      },
    });
  }, [runMutation]);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.rules.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading rules"
      >
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-16 bg-surface-1 rounded-cb-2" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.inbox.rules.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading rules" />
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

  const mutationPending =
    toggleMutation.isPending || deleteMutation.isPending || runMutation.isPending;

  return (
    <div data-testid="settings.inbox.rules.root">
      <div className="flex items-start justify-between gap-4 mb-4">
        <p className="text-[12.5px] text-t3">
          Rules automatically filter, tag, and sort incoming emails.
        </p>
        {!addingRule && (
          <button
            type="button"
            onClick={() => setAddingRule(true)}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 shrink-0",
              "bg-surface-1 border border-line text-[12.5px] text-t2",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            )}
          >
            <Plus size={13} strokeWidth={2} aria-hidden="true" />
            New rule
          </button>
        )}
      </div>

      <section className="bg-surface-1 cb-raised rounded-cb-2">
        {data.length === 0 && !addingRule ? (
          <div className="py-10 text-center">
            <p className="text-[13px] text-t3">No rules yet. Add your first rule above.</p>
          </div>
        ) : (
          data.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              onToggle={handleToggle}
              onDelete={handleDelete}
              onRun={handleRun}
              isPending={mutationPending}
              runSuccess={lastRunId !== null}
            />
          ))
        )}

        {addingRule && <AddRuleForm onDone={() => setAddingRule(false)} />}
      </section>

      {lastRunId && (
        <p className="text-[12px] text-money-text mt-2" role="status">
          Rule queued (run {lastRunId}). Changes will apply shortly.
        </p>
      )}
    </div>
  );
};
