/**
 * TagsSection — Inbox Settings → Tags tab.
 *
 * Shows workspace tags grouped as: visible / hidden-system / hidden-filtered.
 * Actions: create, rename/recolor, hide/unhide, delete.
 * Merge is listed as a pending action (backend supports it; UI deferred to follow-up).
 */
import { type FC, useState, useCallback } from "react";
import { Plus, Eye, EyeOff, Trash2, Check, RotateCcw, Pencil, X } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useTagsQuery,
  useCreateTagMutation,
  useUpdateTagMutation,
  useDeleteTagMutation,
  useToggleTagHiddenMutation,
} from "~/modules/settings/api/use-inbox-tags";
import type { Tag } from "./types";
import { cn } from "~/lib/utils";

// ── Color swatch ──────────────────────────────────────────────────────────────

const ColorSwatch: FC<{ color: string | null }> = ({ color }) => (
  <span
    className="inline-block w-3 h-3 rounded-full shrink-0 border border-line"
    style={{ backgroundColor: color ?? undefined }}
    aria-hidden="true"
  />
);

// ── Tag row ───────────────────────────────────────────────────────────────────

interface TagRowProps {
  tag: Tag;
  onToggleHidden: (id: number) => void;
  onDelete: (id: number) => void;
  isPending: boolean;
}

const TagRow: FC<TagRowProps> = ({ tag, onToggleHidden, onDelete, isPending }) => {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(tag.name);
  const updateMutation = useUpdateTagMutation();

  const handleSave = useCallback((): void => {
    if (!name.trim() || name === tag.name) { setEditing(false); return; }
    updateMutation.mutate(
      { id: tag.id, name: name.trim() },
      { onSuccess: () => setEditing(false) },
    );
  }, [updateMutation, tag.id, tag.name, name]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent): void => {
    if (e.key === "Enter") handleSave();
    if (e.key === "Escape") { setName(tag.name); setEditing(false); }
  }, [handleSave, tag.name]);

  return (
    <div className="flex items-center gap-3 py-3 px-4 sm:px-5 border-b border-line last:border-b-0">
      <ColorSwatch color={tag.color} />

      {editing ? (
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleSave}
          className={cn(
            "flex-1 min-w-0 px-2 py-1 rounded-cb-1 text-[13px]",
            "bg-surface-3 border border-line text-t1",
            "focus:outline-none focus:ring-2 focus:ring-[--ring]",
          )}
        />
      ) : (
        <span className="flex-1 min-w-0 text-[13px] text-t1 truncate">
          {tag.name}
        </span>
      )}

      {tag.message_count !== null && tag.message_count !== undefined && (
        <span className="text-[11.5px] text-t4 tabular-nums shrink-0">
          {tag.message_count}
        </span>
      )}

      <div className="flex items-center gap-1 shrink-0">
        {!editing && (
          <button
            type="button"
            onClick={() => setEditing(true)}
            title="Rename"
            disabled={isPending || tag.system_label}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2",
              "transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
            aria-label={`Rename ${tag.name}`}
          >
            <Pencil size={13} strokeWidth={2} aria-hidden="true" />
          </button>
        )}
        {editing && (
          <>
            <button
              type="button"
              onClick={handleSave}
              disabled={updateMutation.isPending}
              className="p-1.5 rounded-cb-1 text-money-text hover:bg-surface-2 transition-colors duration-[--cb-dur]"
              aria-label="Save name"
            >
              <Check size={13} strokeWidth={2.5} aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={() => { setName(tag.name); setEditing(false); }}
              className="p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2 transition-colors duration-[--cb-dur]"
              aria-label="Cancel"
            >
              <X size={13} strokeWidth={2} aria-hidden="true" />
            </button>
          </>
        )}
        <button
          type="button"
          onClick={() => onToggleHidden(tag.id)}
          title={tag.hidden ? "Unhide" : "Hide"}
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 hover:bg-surface-2",
            "text-t4 hover:text-t2 transition-colors duration-[--cb-dur]",
            "disabled:opacity-40 disabled:cursor-not-allowed",
          )}
          aria-label={tag.hidden ? `Unhide ${tag.name}` : `Hide ${tag.name}`}
        >
          {tag.hidden ? (
            <Eye size={13} strokeWidth={2} aria-hidden="true" />
          ) : (
            <EyeOff size={13} strokeWidth={2} aria-hidden="true" />
          )}
        </button>
        {!tag.system_label && (
          <button
            type="button"
            onClick={() => onDelete(tag.id)}
            title="Delete"
            disabled={isPending}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-2",
              "transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
            aria-label={`Delete ${tag.name}`}
          >
            <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
          </button>
        )}
      </div>
    </div>
  );
};

// ── Add tag form ──────────────────────────────────────────────────────────────

const AddTagForm: FC<{ onDone: () => void }> = ({ onDone }) => {
  const [name, setName] = useState("");
  const createMutation = useCreateTagMutation();

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
        placeholder="Tag name"
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

// ── Tag group block ───────────────────────────────────────────────────────────

interface TagGroupBlockProps {
  title: string;
  tags: Tag[];
  onToggleHidden: (id: number) => void;
  onDelete: (id: number) => void;
  mutationPending: boolean;
}

const TagGroupBlock: FC<TagGroupBlockProps> = ({
  title,
  tags,
  onToggleHidden,
  onDelete,
  mutationPending,
}) => {
  if (tags.length === 0) return null;
  return (
    <section className="bg-surface-1 cb-raised rounded-cb-2 mb-4">
      <div className="px-4 py-2.5 sm:px-5 border-b border-line">
        <h3 className="text-[12px] font-[600] text-t3 uppercase tracking-wide">
          {title}
        </h3>
      </div>
      {tags.map((tag) => (
        <TagRow
          key={tag.id}
          tag={tag}
          onToggleHidden={onToggleHidden}
          onDelete={onDelete}
          isPending={mutationPending}
        />
      ))}
    </section>
  );
};

// ── TagsSection ───────────────────────────────────────────────────────────────

export const TagsSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useTagsQuery();
  const toggleHiddenMutation = useToggleTagHiddenMutation();
  const deleteMutation = useDeleteTagMutation();
  const [addingTag, setAddingTag] = useState(false);

  const handleToggleHidden = useCallback((id: number): void => {
    toggleHiddenMutation.mutate(id);
  }, [toggleHiddenMutation]);

  const handleDelete = useCallback((id: number): void => {
    deleteMutation.mutate(id);
  }, [deleteMutation]);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.tags.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading tags"
      >
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-12 bg-surface-1 rounded-cb-2" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.inbox.tags.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading tags" />
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
    toggleHiddenMutation.isPending || deleteMutation.isPending;

  return (
    <div data-testid="settings.inbox.tags.root">
      <div className="flex items-center justify-between mb-4">
        <p className="text-[12.5px] text-t3">
          Tags classify incoming emails. Visible tags appear as filter chips in the inbox.
          {data.pending_review_count > 0 && (
            <> · <span className="text-people-text">{data.pending_review_count} pending review</span></>
          )}
        </p>
        {!addingTag && (
          <button
            type="button"
            onClick={() => setAddingTag(true)}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 shrink-0",
              "bg-surface-1 border border-line text-[12.5px] text-t2",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            )}
          >
            <Plus size={13} strokeWidth={2} aria-hidden="true" />
            New tag
          </button>
        )}
      </div>

      <TagGroupBlock
        title="Visible"
        tags={data.visible}
        onToggleHidden={handleToggleHidden}
        onDelete={handleDelete}
        mutationPending={mutationPending}
      />

      {data.visible.length === 0 && data.hidden_system.length === 0 && data.hidden_filtered.length === 0 && (
        <div className="bg-surface-1 cb-raised rounded-cb-2 py-10 text-center mb-4">
          <p className="text-[13px] text-t3">No tags yet. Add your first tag above.</p>
        </div>
      )}

      <TagGroupBlock
        title="Hidden — system"
        tags={data.hidden_system}
        onToggleHidden={handleToggleHidden}
        onDelete={handleDelete}
        mutationPending={mutationPending}
      />

      <TagGroupBlock
        title="Hidden — filtered"
        tags={data.hidden_filtered}
        onToggleHidden={handleToggleHidden}
        onDelete={handleDelete}
        mutationPending={mutationPending}
      />

      {addingTag && (
        <div className="bg-surface-1 cb-raised rounded-cb-2 mt-2">
          <AddTagForm onDone={() => setAddingTag(false)} />
        </div>
      )}

      {(toggleHiddenMutation.error ?? deleteMutation.error) && (
        <p className="text-[12px] text-danger mt-3" role="alert">
          {[toggleHiddenMutation.error, deleteMutation.error]
            .filter(Boolean)
            .map((e) =>
              e instanceof ApiError ? e.message : "An error occurred",
            )
            .join(" · ")}
        </p>
      )}
    </div>
  );
};
