/**
 * DocumentTypesSection — Inbox Settings → Document types tab.
 *
 * Custom document-type classifications. Shows list with color swatch, name,
 * category, auto_star badge. Create + edit inline. Delete.
 */
import { type FC, useState, useCallback } from "react";
import { Plus, Trash2, RotateCcw, Check } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useDocumentTypesQuery,
  useCreateDocumentTypeMutation,
  useUpdateDocumentTypeMutation,
  useDeleteDocumentTypeMutation,
} from "~/modules/settings/api/use-inbox-document-types";
import type { DocumentType } from "./types";
import { cn } from "~/lib/utils";

// ── Document type form ────────────────────────────────────────────────────────

interface DocumentTypeFormProps {
  initial?: DocumentType;
  onDone: () => void;
}

const DocumentTypeForm: FC<DocumentTypeFormProps> = ({ initial, onDone }) => {
  const [name, setName] = useState(initial?.name ?? "");
  const [category, setCategory] = useState(initial?.category ?? "");
  const [prompt, setPrompt] = useState(initial?.prompt ?? "");
  const [autoStar, setAutoStar] = useState(initial?.auto_star ?? false);
  const createMutation = useCreateDocumentTypeMutation();
  const updateMutation = useUpdateDocumentTypeMutation();

  const isPending = createMutation.isPending || updateMutation.isPending;
  const mutError = createMutation.error ?? updateMutation.error;

  const handleSubmit = useCallback((e: React.FormEvent): void => {
    e.preventDefault();
    if (!name.trim()) return;
    const params = { name: name.trim(), category: category.trim() || undefined, prompt: prompt.trim() || undefined, auto_star: autoStar };
    if (initial) {
      updateMutation.mutate(
        { id: initial.id, document_type: params },
        { onSuccess: () => onDone() },
      );
    } else {
      createMutation.mutate(
        { document_type: params },
        { onSuccess: () => onDone() },
      );
    }
  }, [createMutation, updateMutation, initial, name, category, prompt, autoStar, onDone]);

  const formId = initial ? `dt-form-${initial.id}` : "dt-form-new";

  const inputClass = cn(
    "w-full px-3 py-2 rounded-cb-1 text-[13px]",
    "bg-surface-3 border border-line text-t1 placeholder:text-t4",
    "focus:outline-none focus:ring-2 focus:ring-[--ring]",
    "transition-colors duration-[--cb-dur]",
  );

  return (
    <form onSubmit={handleSubmit} className="p-4 sm:p-5 flex flex-col gap-4">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div className="flex flex-col gap-1.5">
          <label htmlFor={`${formId}-name`} className="text-[12.5px] font-[500] text-t2">Name</label>
          <input
            id={`${formId}-name`}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Invoice"
            className={inputClass}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor={`${formId}-category`} className="text-[12.5px] font-[500] text-t2">Category (optional)</label>
          <input
            id={`${formId}-category`}
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            placeholder="e.g. financial"
            className={inputClass}
          />
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${formId}-prompt`} className="text-[12.5px] font-[500] text-t2">Classification prompt (optional)</label>
        <textarea
          id={`${formId}-prompt`}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Describe what qualifies as this document type for Scout's classifier."
          rows={3}
          className={cn(inputClass, "resize-y")}
        />
      </div>

      <label className="flex items-center gap-2 cursor-pointer">
        <input
          type="checkbox"
          checked={autoStar}
          onChange={(e) => setAutoStar(e.target.checked)}
          className="accent-[--ring]"
        />
        <span className="text-[13px] text-t2">Auto-star documents of this type</span>
      </label>

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
          {isPending ? "Saving…" : initial ? "Save" : "Add type"}
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

// ── Document type row ─────────────────────────────────────────────────────────

interface DocTypeRowProps {
  docType: DocumentType;
  onDelete: (id: number) => void;
  isPending: boolean;
}

const DocTypeRow: FC<DocTypeRowProps> = ({ docType, onDelete, isPending }) => {
  const [editing, setEditing] = useState(false);
  const [saved, setSaved] = useState(false);

  if (editing) {
    return (
      <div className="border-b border-line last:border-b-0">
        <DocumentTypeForm
          initial={docType}
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
    <div className="flex items-center gap-3 py-3 px-4 sm:px-5 border-b border-line last:border-b-0">
      {docType.color && (
        <span
          className="inline-block w-3 h-3 rounded-full shrink-0 border border-line"
          style={{ backgroundColor: docType.color }}
          aria-hidden="true"
        />
      )}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[13px] font-[500] text-t1">{docType.name}</span>
          {docType.category && (
            <span className="text-[11.5px] text-t4">{docType.category}</span>
          )}
          {docType.auto_star && (
            <span className="text-[11px] px-1.5 py-0.5 rounded-cb-1 bg-surface-2 border border-line text-t4">
              auto-star
            </span>
          )}
          {saved && (
            <span className="inline-flex items-center gap-1 text-[11px] text-money-text">
              <Check size={10} strokeWidth={2.5} aria-hidden="true" />
              Saved
            </span>
          )}
        </div>
        {docType.prompt && (
          <p className="text-[11.5px] text-t3 mt-0.5 truncate">{docType.prompt}</p>
        )}
      </div>
      <div className="flex items-center gap-1 shrink-0">
        <button
          type="button"
          onClick={() => setEditing(true)}
          className={cn(
            "px-2.5 py-1.5 rounded-cb-1 text-[12px] text-t3",
            "bg-surface-1 border border-line hover:bg-surface-2",
            "transition-colors duration-[--cb-dur]",
          )}
          aria-label={`Edit ${docType.name}`}
        >
          Edit
        </button>
        <button
          type="button"
          onClick={() => onDelete(docType.id)}
          title="Delete"
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-2",
            "transition-colors duration-[--cb-dur]",
            "disabled:opacity-40 disabled:cursor-not-allowed",
          )}
          aria-label={`Delete ${docType.name}`}
        >
          <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
};

// ── DocumentTypesSection ──────────────────────────────────────────────────────

export const DocumentTypesSection: FC = () => {
  const { data, isFetching, isError, error, refetch } = useDocumentTypesQuery();
  const deleteMutation = useDeleteDocumentTypeMutation();
  const [adding, setAdding] = useState(false);

  const handleDelete = useCallback((id: number): void => {
    deleteMutation.mutate(id);
  }, [deleteMutation]);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.inbox.document-types.loading"
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading document types"
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
        data-testid="settings.inbox.document-types.error"
        className="flex flex-col items-center justify-center gap-3 py-12 text-center"
      >
        <Visor state="asleep" size={26} label="Error loading document types" />
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
    <div data-testid="settings.inbox.document-types.root">
      <div className="flex items-start justify-between gap-4 mb-4">
        <p className="text-[12.5px] text-t3">
          Scout classifies email attachments into these types.
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
            New type
          </button>
        )}
      </div>

      <section className="bg-surface-1 cb-raised rounded-cb-2">
        {data.length === 0 && !adding ? (
          <div className="py-10 text-center">
            <p className="text-[13px] text-t3">No custom document types yet.</p>
          </div>
        ) : (
          data.map((dt) => (
            <DocTypeRow
              key={dt.id}
              docType={dt}
              onDelete={handleDelete}
              isPending={deleteMutation.isPending}
            />
          ))
        )}

        {adding && (
          <div className={data.length > 0 ? "border-t border-line" : ""}>
            <DocumentTypeForm onDone={() => setAdding(false)} />
          </div>
        )}
      </section>
    </div>
  );
};
