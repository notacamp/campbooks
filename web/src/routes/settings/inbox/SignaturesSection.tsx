/**
 * SignaturesSection — Inbox Settings → Signatures tab.
 *
 * List email signatures with default badge, set_default, edit (name + content), delete.
 * Create opens an inline form.
 */
import { type FC, useState, useCallback } from "react";
import { Plus, Star, StarOff, Trash2, RotateCcw, Check } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useSignaturesQuery,
  useCreateSignatureMutation,
  useUpdateSignatureMutation,
  useDeleteSignatureMutation,
  useSetDefaultSignatureMutation,
} from "~/modules/settings/api/use-inbox-signatures";
import type { Signature } from "./types";
import { cn } from "~/lib/utils";

// ── Signature form (create / edit) ────────────────────────────────────────────

interface SignatureFormProps {
  initial?: Signature;
  onDone: () => void;
}

const SignatureForm: FC<SignatureFormProps> = ({ initial, onDone }) => {
  const [name, setName] = useState(initial?.name ?? "");
  const [content, setContent] = useState(initial?.content ?? "");
  const createMutation = useCreateSignatureMutation();
  const updateMutation = useUpdateSignatureMutation();

  const isPending = createMutation.isPending || updateMutation.isPending;
  const mutError = createMutation.error ?? updateMutation.error;

  const handleSubmit = useCallback((e: React.FormEvent): void => {
    e.preventDefault();
    if (!name.trim()) return;
    if (initial) {
      updateMutation.mutate(
        { id: initial.id, signature: { name: name.trim(), content } },
        { onSuccess: () => onDone() },
      );
    } else {
      createMutation.mutate(
        { signature: { name: name.trim(), content } },
        { onSuccess: () => onDone() },
      );
    }
  }, [createMutation, updateMutation, initial, name, content, onDone]);

  const inputClass = cn(
    "w-full px-3 py-2 rounded-cb-1 text-[13px]",
    "bg-surface-3 border border-line text-t1 placeholder:text-t4",
    "focus:outline-none focus:ring-2 focus:ring-[--ring]",
    "transition-colors duration-[--cb-dur]",
  );

  const formId = initial ? `sig-form-${initial.id}` : "sig-form-new";

  return (
    <form onSubmit={handleSubmit} className="p-4 sm:p-5 flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${formId}-name`} className="text-[12.5px] font-[500] text-t2">Signature name</label>
        <input
          id={`${formId}-name`}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. Work, Personal"
          className={inputClass}
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${formId}-content`} className="text-[12.5px] font-[500] text-t2">Content</label>
        <textarea
          id={`${formId}-content`}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Your signature text (plain text or HTML)"
          rows={5}
          className={cn(inputClass, "resize-y")}
        />
      </div>
      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={!name.trim() || isPending}
          className={cn(
            "px-4 py-2 rounded-cb-1 text-[13px] font-[500]",
            "bg-[--primary] text-[--primary-foreground]",
            "disabled:opacity-50 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          {isPending ? "Saving…" : initial ? "Save" : "Add signature"}
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
        {mutError && (
          <p className="text-[12px] text-danger" role="alert">
            {mutError instanceof ApiError ? mutError.message : "Error"}
          </p>
        )}
      </div>
    </form>
  );
};

// ── Signature row ─────────────────────────────────────────────────────────────

interface SignatureRowProps {
  sig: Signature;
  onSetDefault: (id: number) => void;
  onDelete: (id: number) => void;
  isPending: boolean;
}

const SignatureRow: FC<SignatureRowProps> = ({
  sig,
  onSetDefault,
  onDelete,
  isPending,
}) => {
  const [editing, setEditing] = useState(false);
  const [saved, setSaved] = useState(false);

  if (editing) {
    return (
      <div className="border-b border-line last:border-b-0">
        <SignatureForm
          initial={sig}
          onDone={() => {
            setEditing(false);
            setSaved(true);
            setTimeout(() => setSaved(false), 2000);
          }}
        />
      </div>
    );
  }

  return (
    <div className="flex items-start gap-3 py-4 px-4 sm:px-5 border-b border-line last:border-b-0">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[13px] font-[500] text-t1">{sig.name}</span>
          {sig.is_default && (
            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-cb-1 bg-surface-2 border border-line text-[11px] text-t3">
              <Star size={10} strokeWidth={2} aria-hidden="true" />
              Default
            </span>
          )}
          {saved && (
            <span className="inline-flex items-center gap-1 text-[11px] text-money-text">
              <Check size={10} strokeWidth={2.5} aria-hidden="true" />
              Saved
            </span>
          )}
        </div>
        <p className="text-[12px] text-t3 mt-0.5 line-clamp-2">
          {sig.content || <em className="italic">No content</em>}
        </p>
      </div>

      <div className="flex items-center gap-1 shrink-0 pt-0.5">
        <button
          type="button"
          onClick={() => setEditing(true)}
          className={cn(
            "px-2.5 py-1.5 rounded-cb-1 text-[12px] text-t3",
            "bg-surface-1 border border-line hover:bg-surface-2",
            "transition-colors duration-[--cb-dur]",
          )}
          aria-label={`Edit ${sig.name}`}
        >
          Edit
        </button>
        {!sig.is_default && (
          <button
            type="button"
            onClick={() => onSetDefault(sig.id)}
            title="Set as default"
            disabled={isPending}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2",
              "transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
            aria-label={`Set ${sig.name} as default`}
          >
            <StarOff size={13} strokeWidth={2} aria-hidden="true" />
          </button>
        )}
        <button
          type="button"
          onClick={() => onDelete(sig.id)}
          title="Delete"
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-2",
            "transition-colors duration-[--cb-dur]",
            "disabled:opacity-40 disabled:cursor-not-allowed",
          )}
          aria-label={`Delete ${sig.name}`}
        >
          <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
};

// ── SignaturesSection ─────────────────────────────────────────────────────────

export const SignaturesSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useSignaturesQuery();
  const setDefaultMutation = useSetDefaultSignatureMutation();
  const deleteMutation = useDeleteSignatureMutation();
  const [adding, setAdding] = useState(false);

  const handleSetDefault = useCallback((id: number): void => {
    setDefaultMutation.mutate(id);
  }, [setDefaultMutation]);

  const handleDelete = useCallback((id: number): void => {
    deleteMutation.mutate(id);
  }, [deleteMutation]);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.signatures.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading signatures"
      >
        {[0, 1].map((i) => (
          <div key={i} className="h-16 bg-surface-1 rounded-cb-2" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.inbox.signatures.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading signatures" />
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

  const mutationPending = setDefaultMutation.isPending || deleteMutation.isPending;

  return (
    <div data-testid="settings.inbox.signatures.root">
      <div className="flex items-start justify-between gap-4 mb-4">
        <p className="text-[12.5px] text-t3">
          Email signatures appended to outgoing messages. The default is selected automatically.
        </p>
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
            New signature
          </button>
        )}
      </div>

      <section className="bg-surface-1 cb-raised rounded-cb-2">
        {data.length === 0 && !adding ? (
          <div className="py-10 text-center">
            <p className="text-[13px] text-t3">No signatures yet.</p>
          </div>
        ) : (
          data.map((sig) => (
            <SignatureRow
              key={sig.id}
              sig={sig}
              onSetDefault={handleSetDefault}
              onDelete={handleDelete}
              isPending={mutationPending}
            />
          ))
        )}

        {adding && (
          <div className={data.length > 0 ? "border-t border-line" : ""}>
            <SignatureForm onDone={() => setAdding(false)} />
          </div>
        )}
      </section>
    </div>
  );
};
