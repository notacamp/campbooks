/**
 * TagGroupsSection — Inbox Settings → Tag groups tab.
 *
 * Shows inbox group (stream) definitions: name, associated tags, filter rules.
 * Create opens a form; delete removes a group (tags are unassigned, not deleted).
 * Edit (renaming / changing tags / rules) is a follow-up — flagged inline.
 */
import { type FC, useState, useCallback } from "react";
import { Plus, Trash2, RotateCcw, ChevronDown, ChevronUp } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useTagGroupsQuery,
  useCreateTagGroupMutation,
  useDeleteTagGroupMutation,
} from "~/modules/settings/api/use-inbox-tag-groups";
import type { TagGroup } from "./types";
import { cn } from "~/lib/utils";

// ── Tag group row ─────────────────────────────────────────────────────────────

interface TagGroupRowProps {
  group: TagGroup;
  onDelete: (name: string) => void;
  isPending: boolean;
}

const TagGroupRow: FC<TagGroupRowProps> = ({ group, onDelete, isPending }) => {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border-b border-line last:border-b-0">
      <div className="flex items-center gap-3 py-3 px-4 sm:px-5">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-[500] text-t1">{group.name}</span>
            <span className="text-[11.5px] text-t4">
              {group.tag_names.length > 0
                ? `${group.tag_names.length} tag${group.tag_names.length === 1 ? "" : "s"}`
                : "no tags"}
              {group.rules.length > 0 && ` · ${group.rules.length} rule${group.rules.length === 1 ? "" : "s"}`}
            </span>
          </div>
          {group.tag_names.length > 0 && (
            <p className="text-[11.5px] text-t3 mt-0.5 truncate">
              {group.tag_names.join(", ")}
            </p>
          )}
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {group.rules.length > 0 && (
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className={cn(
                "p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label={expanded ? "Collapse rules" : "Expand rules"}
            >
              {expanded ? (
                <ChevronUp size={13} strokeWidth={2} aria-hidden="true" />
              ) : (
                <ChevronDown size={13} strokeWidth={2} aria-hidden="true" />
              )}
            </button>
          )}
          <button
            type="button"
            onClick={() => onDelete(group.name)}
            title="Delete group"
            disabled={isPending}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-2",
              "transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
            aria-label={`Delete group ${group.name}`}
          >
            <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
          </button>
        </div>
      </div>

      {expanded && group.rules.length > 0 && (
        <div className="px-4 sm:px-5 pb-3 flex flex-col gap-1.5">
          {group.rules.map((rule, i) => (
            <p key={i} className="text-[12px] text-t3">
              <span className="font-[500] text-t2">{rule.rule_type}:</span> {rule.value}
            </p>
          ))}
        </div>
      )}
    </div>
  );
};

// ── Add group form ────────────────────────────────────────────────────────────

const AddGroupForm: FC<{ onDone: () => void }> = ({ onDone }) => {
  const [name, setName] = useState("");
  const createMutation = useCreateTagGroupMutation();

  const handleSubmit = useCallback((e: React.FormEvent): void => {
    e.preventDefault();
    if (!name.trim()) return;
    createMutation.mutate(
      { name: name.trim() },
      { onSuccess: () => { setName(""); onDone(); } },
    );
  }, [createMutation, name, onDone]);

  return (
    <form
      onSubmit={handleSubmit}
      className="flex items-center gap-2 py-3 px-4 sm:px-5 border-t border-line"
    >
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Group name"
        className={cn(
          "flex-1 min-w-0 px-3 py-2 rounded-cb-1 text-[13px]",
          "bg-surface-3 border border-line text-t1 placeholder:text-t4",
          "focus:outline-none focus:ring-2 focus:ring-[--ring]",
          "transition-colors duration-[--cb-dur]",
        )}
      />
      <button
        type="submit"
        disabled={!name.trim() || createMutation.isPending}
        className={cn(
          "px-3 py-2 rounded-cb-1 text-[12.5px] font-[500]",
          "bg-[--primary] text-[--primary-foreground]",
          "disabled:opacity-50 disabled:cursor-not-allowed",
          "transition-colors duration-[--cb-dur]",
        )}
      >
        {createMutation.isPending ? "Adding…" : "Add"}
      </button>
      <button
        type="button"
        onClick={onDone}
        className={cn(
          "px-3 py-2 rounded-cb-1 text-[12.5px]",
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
    </form>
  );
};

// ── TagGroupsSection ──────────────────────────────────────────────────────────

export const TagGroupsSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useTagGroupsQuery();
  const deleteMutation = useDeleteTagGroupMutation();
  const [adding, setAdding] = useState(false);

  const handleDelete = useCallback((name: string): void => {
    deleteMutation.mutate(name);
  }, [deleteMutation]);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.tag-groups.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading tag groups"
      >
        {[0, 1].map((i) => (
          <div key={i} className="h-12 bg-surface-1 rounded-cb-2" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.inbox.tag-groups.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading tag groups" />
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

  return (
    <div data-testid="settings.inbox.tag-groups.root">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div>
          <p className="text-[12.5px] text-t3">
            Groups bundle tags into named streams for the inbox sidebar.
          </p>
          <p className="text-[11.5px] text-t4 mt-0.5">
            Note: editing existing groups (rename, reassign tags) is not yet available in the SPA — use the classic settings for now.
          </p>
        </div>
        {!adding && (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 shrink-0",
              "bg-surface-1 border border-line text-[12.5px] text-t2",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            )}
          >
            <Plus size={13} strokeWidth={2} aria-hidden="true" />
            New group
          </button>
        )}
      </div>

      <section className="bg-surface-1 cb-raised rounded-cb-2">
        {data.length === 0 && !adding ? (
          <div className="py-10 text-center">
            <p className="text-[13px] text-t3">No tag groups yet.</p>
          </div>
        ) : (
          data.map((group) => (
            <TagGroupRow
              key={group.name}
              group={group}
              onDelete={handleDelete}
              isPending={deleteMutation.isPending}
            />
          ))
        )}

        {adding && <AddGroupForm onDone={() => setAdding(false)} />}
      </section>
    </div>
  );
};
