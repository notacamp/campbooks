/**
 * AiPage — Settings → AI at /settings/ai.
 *
 * Three sections:
 *   1. Processing mode — managed vs BYO toggle
 *   2. Adapters — BYO provider adapters (add / edit / remove)
 *   3. AI prompts — customisable per-purpose guidance (list, edit, reset)
 *
 * Endpoints:
 *   GET    /api/app/settings/ai                         — overview
 *   POST   /api/app/settings/ai/switch_mode             — managed | byo
 *   PATCH  /api/app/settings/ai/embeddings              — embedding model
 *   GET    /api/app/settings/ai_adapters                — list (in overview)
 *   POST   /api/app/settings/ai_adapters                — create adapter
 *   PATCH  /api/app/settings/ai_adapters/:id            — update adapter
 *   DELETE /api/app/settings/ai_adapters/:id            — delete adapter
 *   GET    /api/app/settings/ai_prompts                 — list all purposes
 *   PATCH  /api/app/settings/ai_prompts/:purpose        — update / reset
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check, Plus, Trash2, Pencil, X } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import { cn } from "~/lib/utils";
import { useAiQuery } from "~/modules/settings/api/use-ai-query";
import type { AiAdapterData } from "~/modules/settings/api/use-ai-query";
import {
  useAiSwitchModeMutation,
  useAiEmbeddingsMutation,
} from "~/modules/settings/api/use-ai-mutation";
import {
  useCreateAdapterMutation,
  useUpdateAdapterMutation,
  useDeleteAdapterMutation,
} from "~/modules/settings/api/use-ai-adapters-mutation";
import { useAiPromptsQuery } from "~/modules/settings/api/use-ai-prompts-query";
import { useUpdatePromptMutation } from "~/modules/settings/api/use-ai-prompts-mutation";

// ── Shared helpers (local) ────────────────────────────────────────────────────

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

// ── Toggle ────────────────────────────────────────────────────────────────────

interface ToggleProps {
  checked: boolean;
  onChange: () => void;
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
    onClick={onChange}
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

// ── Input ─────────────────────────────────────────────────────────────────────

const InlineInput: FC<{
  value: string;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}> = ({ value, onChange, type = "text", placeholder, disabled, className }) => (
  <input
    type={type}
    value={value}
    onChange={(e) => onChange(e.target.value)}
    placeholder={placeholder}
    disabled={disabled}
    className={cn(
      "w-full px-3 py-2 rounded-cb-1 text-[13px]",
      "bg-surface-3 border border-line",
      "text-t1 placeholder:text-t4",
      "focus:outline-none focus:ring-2 focus:ring-[--ring]",
      "disabled:opacity-40 disabled:cursor-not-allowed",
      "transition-colors duration-[--cb-dur]",
      className,
    )}
  />
);

// ── Save button ───────────────────────────────────────────────────────────────

const SaveButton: FC<{
  onClick: () => void;
  isPending: boolean;
  saved: boolean;
  label?: string;
}> = ({ onClick, isPending, saved, label = "Save" }) => (
  <button
    type="button"
    onClick={onClick}
    disabled={isPending || saved}
    className={cn(
      "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 text-[12.5px] font-[500]",
      saved
        ? "bg-surface-1 border border-line text-t4"
        : "bg-[--primary] text-[--primary-foreground]",
      "disabled:opacity-60 disabled:cursor-not-allowed",
      "transition-colors duration-[--cb-dur]",
    )}
  >
    {saved ? (
      <>
        <Check size={12} strokeWidth={2.5} aria-hidden="true" />
        Saved
      </>
    ) : isPending ? (
      "Saving…"
    ) : (
      label
    )}
  </button>
);

// ── Loading skeleton ──────────────────────────────────────────────────────────

const LoadingSkeleton: FC<{ label: string }> = ({ label }) => (
  <div
    data-testid="settings.page.loading"
    className="flex flex-col gap-4 max-w-2xl animate-pulse"
    aria-busy="true"
    aria-label={label}
  >
    <div className="h-6 w-32 bg-surface-1 rounded-cb-1" />
    {[0, 1, 2, 3].map((i) => (
      <div key={i} className="h-14 bg-surface-1 rounded-cb-1" />
    ))}
  </div>
);

// ── Error state ───────────────────────────────────────────────────────────────

const ErrorState: FC<{ error: Error | null; label: string; onRetry: () => void }> = ({
  error,
  label,
  onRetry,
}) => (
  <div
    data-testid="settings.page.error"
    className="flex flex-col items-center justify-center gap-3 py-16 text-center"
  >
    <Visor state="asleep" size={28} label={label} />
    <p className="text-[13px] text-t2">
      {error instanceof Error ? error.message : "Something went wrong"}
    </p>
    <button
      type="button"
      onClick={onRetry}
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

// ── Adapter form ──────────────────────────────────────────────────────────────

interface AdapterFormState {
  name: string;
  provider: string;
  api_key: string;
  endpoint_url: string;
  enabled: boolean;
}

const EMPTY_FORM: AdapterFormState = {
  name: "",
  provider: "openai",
  api_key: "",
  endpoint_url: "",
  enabled: true,
};

const PROVIDER_OPTIONS = [
  { value: "openai",    label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "mistral",   label: "Mistral" },
  { value: "ollama",    label: "Ollama (self-hosted)" },
  { value: "other",     label: "Other / OpenAI-compatible" },
];

interface AdapterFormProps {
  initial?: AiAdapterData;
  onCancel: () => void;
  onSaved: () => void;
}

const AdapterForm: FC<AdapterFormProps> = ({ initial, onCancel, onSaved }) => {
  const createMutation = useCreateAdapterMutation();
  const updateMutation = useUpdateAdapterMutation();

  const [form, setForm] = useState<AdapterFormState>(
    initial
      ? {
          name:         initial.name,
          provider:     initial.provider,
          api_key:      "",
          endpoint_url: initial.endpoint_url ?? "",
          enabled:      initial.enabled,
        }
      : EMPTY_FORM,
  );

  const setField = <K extends keyof AdapterFormState>(
    key: K,
    value: AdapterFormState[K],
  ): void => setForm((prev) => ({ ...prev, [key]: value }));

  const isPending = createMutation.isPending || updateMutation.isPending;
  const mutationError = createMutation.error ?? updateMutation.error;

  const handleSubmit = (): void => {
    if (initial) {
      updateMutation.mutate(
        {
          id: initial.id,
          ai_adapter: {
            name:         form.name,
            api_key:      form.api_key || undefined,
            endpoint_url: form.endpoint_url || undefined,
            enabled:      form.enabled,
          },
        },
        { onSuccess: onSaved },
      );
    } else {
      createMutation.mutate(
        {
          ai_adapter: {
            name:         form.name,
            provider:     form.provider,
            api_key:      form.api_key || undefined,
            endpoint_url: form.endpoint_url || undefined,
            enabled:      form.enabled,
          },
        },
        { onSuccess: onSaved },
      );
    }
  };

  return (
    <div className="flex flex-col gap-3 p-4 bg-surface-2 rounded-cb-1 border border-line">
      <p className="text-[13px] font-[600] text-t1">
        {initial ? "Edit adapter" : "Add adapter"}
      </p>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div className="flex flex-col gap-1">
          <p className="text-[12px] text-t2 font-[500]">Name</p>
          <InlineInput
            value={form.name}
            onChange={(v) => setField("name", v)}
            placeholder="My OpenAI adapter"
          />
        </div>

        {!initial && (
          <div className="flex flex-col gap-1">
            <p className="text-[12px] text-t2 font-[500]">Provider</p>
            <select
              value={form.provider}
              onChange={(e) => setField("provider", e.target.value)}
              className={cn(
                "w-full px-3 py-2 rounded-cb-1 text-[13px]",
                "bg-surface-3 border border-line text-t1",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label="Provider"
            >
              {PROVIDER_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="flex flex-col gap-1">
          <p className="text-[12px] text-t2 font-[500]">
            API key
            {initial && (
              <span className="ml-1 text-t4 font-[400]">
                (leave blank to keep current)
              </span>
            )}
          </p>
          <InlineInput
            type="password"
            value={form.api_key}
            onChange={(v) => setField("api_key", v)}
            placeholder={initial ? "••••••••" : "sk-…"}
          />
        </div>

        <div className="flex flex-col gap-1">
          <p className="text-[12px] text-t2 font-[500]">
            Endpoint URL
            <span className="ml-1 text-t4 font-[400]">(optional)</span>
          </p>
          <InlineInput
            value={form.endpoint_url}
            onChange={(v) => setField("endpoint_url", v)}
            placeholder="https://api.openai.com/v1"
          />
        </div>
      </div>

      <div className="flex items-center gap-2">
        <Toggle
          checked={form.enabled}
          onChange={() => setField("enabled", !form.enabled)}
          label="Enabled"
        />
        <span className="text-[12.5px] text-t2">
          {form.enabled ? "Enabled" : "Disabled"}
        </span>
      </div>

      <FieldError error={mutationError} />

      <div className="flex items-center gap-2 pt-1">
        <SaveButton
          onClick={handleSubmit}
          isPending={isPending}
          saved={false}
          label={initial ? "Update" : "Add adapter"}
        />
        <button
          type="button"
          onClick={onCancel}
          disabled={isPending}
          className={cn(
            "px-3 py-1.5 rounded-cb-1 text-[12.5px] text-t2",
            "bg-surface-1 border border-line",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            "disabled:opacity-60 disabled:cursor-not-allowed",
          )}
        >
          Cancel
        </button>
      </div>
    </div>
  );
};

// ── Adapter row ───────────────────────────────────────────────────────────────

interface AdapterRowProps {
  adapter: AiAdapterData;
  onEdit: (a: AiAdapterData) => void;
}

const AdapterRow: FC<AdapterRowProps> = ({ adapter, onEdit }) => {
  const deleteMutation = useDeleteAdapterMutation();
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div className="flex items-center gap-3 py-3 border-b border-line last:border-b-0">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[13px] font-[500] text-t1 truncate">{adapter.name}</span>
          <span className="text-[11.5px] text-t4 capitalize">{adapter.provider}</span>
          {adapter.managed && (
            <span className="text-[11px] text-t4 bg-surface-2 px-1.5 py-0.5 rounded-cb-1 border border-line">
              managed
            </span>
          )}
          {!adapter.enabled && (
            <span className="text-[11px] text-danger bg-danger/10 px-1.5 py-0.5 rounded-cb-1">
              disabled
            </span>
          )}
          {adapter.in_use && (
            <span className="text-[11px] text-accent-cb bg-accent-cb/10 px-1.5 py-0.5 rounded-cb-1">
              in use
            </span>
          )}
          {adapter.has_api_key && (
            <span className="text-[11px] text-t4 bg-surface-2 px-1.5 py-0.5 rounded-cb-1 border border-line">
              key set
            </span>
          )}
        </div>
        {adapter.endpoint_url && (
          <p className="text-[11.5px] text-t4 mt-0.5 truncate">{adapter.endpoint_url}</p>
        )}
      </div>

      {!adapter.managed && (
        <div className="flex items-center gap-1 shrink-0">
          {confirmDelete ? (
            <>
              <button
                type="button"
                onClick={() => deleteMutation.mutate({ id: adapter.id })}
                disabled={deleteMutation.isPending}
                className="text-[12px] text-danger px-2 py-1 rounded-cb-1 hover:bg-danger/10 transition-colors duration-[--cb-dur]"
              >
                {deleteMutation.isPending ? "Removing…" : "Confirm remove"}
              </button>
              <button
                type="button"
                onClick={() => setConfirmDelete(false)}
                className="p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2 transition-colors duration-[--cb-dur]"
                aria-label="Cancel"
              >
                <X size={13} strokeWidth={2} aria-hidden="true" />
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                onClick={() => onEdit(adapter)}
                className="p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2 transition-colors duration-[--cb-dur]"
                aria-label={`Edit ${adapter.name}`}
              >
                <Pencil size={13} strokeWidth={2} aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => setConfirmDelete(true)}
                className="p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-danger/10 transition-colors duration-[--cb-dur]"
                aria-label={`Remove ${adapter.name}`}
              >
                <Trash2 size={13} strokeWidth={2} aria-hidden="true" />
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
};

// ── Prompt row ────────────────────────────────────────────────────────────────

interface PromptRowProps {
  purpose: string;
  label: string;
  description: string | null;
  instructions: string | null;
  configured: boolean;
}

const PromptRow: FC<PromptRowProps> = ({
  purpose,
  label,
  description,
  instructions,
  configured,
}) => {
  const updateMutation = useUpdatePromptMutation();
  const [editing, setEditing]   = useState(false);
  const [draft,   setDraft]     = useState<string | null>(null);
  const [saved,   setSaved]     = useState(false);

  const effectiveInstructions = draft ?? instructions ?? "";

  const handleSave = (): void => {
    updateMutation.mutate(
      { purpose, instructions: effectiveInstructions },
      {
        onSuccess: () => {
          setDraft(null);
          setEditing(false);
          setSaved(true);
          setTimeout(() => setSaved(false), 2000);
        },
      },
    );
  };

  const handleReset = (): void => {
    updateMutation.mutate(
      { purpose, instructions: "" },
      {
        onSuccess: () => {
          setDraft(null);
          setEditing(false);
          setSaved(true);
          setTimeout(() => setSaved(false), 2000);
        },
      },
    );
  };

  return (
    <div className="py-4 border-b border-line last:border-b-0">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-[13px] font-[500] text-t1">{label}</span>
            {configured && (
              <span className="text-[11px] text-accent-cb bg-accent-cb/10 px-1.5 py-0.5 rounded-cb-1">
                customised
              </span>
            )}
          </div>
          {description && (
            <p className="text-[12px] text-t4 mt-0.5 leading-snug">{description}</p>
          )}
        </div>

        {!editing && (
          <button
            type="button"
            onClick={() => {
              setDraft(instructions ?? "");
              setEditing(true);
            }}
            className="shrink-0 p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2 transition-colors duration-[--cb-dur]"
            aria-label={`Edit ${label} prompt`}
          >
            <Pencil size={13} strokeWidth={2} aria-hidden="true" />
          </button>
        )}
      </div>

      {editing && (
        <div className="mt-3 flex flex-col gap-2">
          <textarea
            value={effectiveInstructions}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Leave blank to use the built-in Scout guidance."
            rows={4}
            className={cn(
              "w-full px-3 py-2 rounded-cb-1 text-[13px] resize-y",
              "bg-surface-3 border border-line",
              "text-t1 placeholder:text-t4",
              "focus:outline-none focus:ring-2 focus:ring-[--ring]",
              "transition-colors duration-[--cb-dur]",
            )}
            aria-label={`Custom guidance for ${label}`}
          />
          <FieldError error={updateMutation.error} />
          <div className="flex items-center gap-2 flex-wrap">
            <SaveButton
              onClick={handleSave}
              isPending={updateMutation.isPending}
              saved={saved}
            />
            {configured && !saved && (
              <button
                type="button"
                onClick={handleReset}
                disabled={updateMutation.isPending}
                className={cn(
                  "px-3 py-1.5 rounded-cb-1 text-[12.5px] text-t3",
                  "bg-surface-1 border border-line",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                  "disabled:opacity-60",
                )}
              >
                Reset to default
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                setEditing(false);
                setDraft(null);
              }}
              className={cn(
                "px-3 py-1.5 rounded-cb-1 text-[12.5px] text-t3",
                "bg-surface-1 border border-line",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              )}
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

// ── AiPage ────────────────────────────────────────────────────────────────────

export const AiPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useAiQuery();
  const {
    data: prompts,
    isFetching: promptsFetching,
    isError: promptsError,
  } = useAiPromptsQuery();

  const switchModeMutation   = useAiSwitchModeMutation();
  const embeddingsMutation   = useAiEmbeddingsMutation();

  const [embeddingModel, setEmbeddingModel] = useState<string | null>(null);
  const [embeddingSaved, setEmbeddingSaved] = useState(false);

  // Adapter form state
  const [showAdapterForm, setShowAdapterForm] = useState(false);
  const [editingAdapter,  setEditingAdapter]  = useState<AiAdapterData | null>(null);

  const effectiveEmbedding = embeddingModel ?? data?.embedding_model ?? "";

  const handleSwitchMode = useCallback(
    (mode: "managed" | "byo"): void => {
      switchModeMutation.mutate({ mode });
    },
    [switchModeMutation],
  );

  const handleSaveEmbedding = useCallback((): void => {
    if (!effectiveEmbedding) return;
    embeddingsMutation.mutate(
      { embedding_model: effectiveEmbedding },
      {
        onSuccess: () => {
          setEmbeddingModel(null);
          setEmbeddingSaved(true);
          setTimeout(() => setEmbeddingSaved(false), 2000);
        },
      },
    );
  }, [embeddingsMutation, effectiveEmbedding]);

  // ── Loading / error ────────────────────────────────────────────────────────

  const isLoading = (isFetching && !data) || (promptsFetching && !prompts);

  if (isLoading) {
    return <LoadingSkeleton label="Loading AI settings" />;
  }

  if ((isError || !data) && (promptsError || !prompts)) {
    return (
      <ErrorState
        error={error}
        label="Error loading AI settings"
        onRetry={() => refetch()}
      />
    );
  }

  // ── Content ────────────────────────────────────────────────────────────────

  const isManaged = data?.mode === "managed";
  const byoAdapters = (data?.adapters ?? []).filter((a) => !a.managed);
  const managedAdapters = (data?.adapters ?? []).filter((a) => a.managed);

  return (
    <div data-testid="settings.page.root" className="max-w-2xl">
      <h1 className="text-[17px] font-[650] text-t1 mb-6">AI</h1>

      {/* ── Processing mode ─────────────────────────────────────────────── */}
      {data && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
          <div className="px-4 py-3 sm:px-5">
            <h2 className="text-[14px] font-[600] text-t1">Processing mode</h2>
            <p className="text-[12px] text-t3 mt-0.5">
              Choose how Scout&apos;s AI calls are fulfilled.
            </p>
          </div>

          <div className="px-4 py-4 sm:px-5 flex flex-col gap-3">
            {/* Managed option */}
            <div
              className={cn(
                "flex items-start gap-4 p-3 rounded-cb-1 border cursor-pointer transition-colors duration-[--cb-dur]",
                isManaged
                  ? "border-accent-cb bg-accent-cb/5"
                  : "border-line hover:bg-surface-2",
                !data.managed_available && "opacity-50 cursor-not-allowed",
              )}
              onClick={() => {
                if (data.managed_available && !isManaged) {
                  handleSwitchMode("managed");
                }
              }}
              role="radio"
              aria-checked={isManaged}
              tabIndex={data.managed_available ? 0 : -1}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  if (data.managed_available && !isManaged) handleSwitchMode("managed");
                }
              }}
            >
              <div
                className={cn(
                  "mt-0.5 w-4 h-4 rounded-full border-2 shrink-0 transition-colors duration-[--cb-dur]",
                  isManaged ? "border-accent-cb bg-accent-cb" : "border-line bg-transparent",
                )}
              />
              <div className="flex-1 min-w-0">
                <p className="text-[13px] font-[500] text-t1">Managed AI</p>
                <p className="text-[12px] text-t3 leading-snug mt-0.5">
                  Campbooks provides the AI models. No API keys needed.
                  {!data.managed_available && (
                    <span className="text-danger ml-1">Not available on this plan.</span>
                  )}
                </p>
              </div>
            </div>

            {/* BYO option */}
            <div
              className={cn(
                "flex items-start gap-4 p-3 rounded-cb-1 border cursor-pointer transition-colors duration-[--cb-dur]",
                !isManaged
                  ? "border-accent-cb bg-accent-cb/5"
                  : "border-line hover:bg-surface-2",
              )}
              onClick={() => {
                if (isManaged) handleSwitchMode("byo");
              }}
              role="radio"
              aria-checked={!isManaged}
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  if (isManaged) handleSwitchMode("byo");
                }
              }}
            >
              <div
                className={cn(
                  "mt-0.5 w-4 h-4 rounded-full border-2 shrink-0 transition-colors duration-[--cb-dur]",
                  !isManaged ? "border-accent-cb bg-accent-cb" : "border-line bg-transparent",
                )}
              />
              <div className="flex-1 min-w-0">
                <p className="text-[13px] font-[500] text-t1">Bring your own API key</p>
                <p className="text-[12px] text-t3 leading-snug mt-0.5">
                  Use your own provider credentials. Add adapters below.
                </p>
              </div>
            </div>

            <FieldError error={switchModeMutation.error} />
          </div>

          {/* Managed adapters (read-only list) */}
          {managedAdapters.length > 0 && (
            <div className="px-4 py-3 sm:px-5">
              <p className="text-[12px] text-t4 mb-2">Managed adapters</p>
              {managedAdapters.map((a) => (
                <AdapterRow key={a.id} adapter={a} onEdit={() => void 0} />
              ))}
            </div>
          )}
        </section>
      )}

      {/* ── Embedding model ──────────────────────────────────────────────── */}
      {data && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
          <div className="px-4 py-3 sm:px-5">
            <h2 className="text-[14px] font-[600] text-t1">Embedding model</h2>
            <p className="text-[12px] text-t3 mt-0.5">
              Used for semantic search and document similarity. Changing this re-embeds your
              workspace in the background.
            </p>
          </div>

          <div className="px-4 py-4 sm:px-5 flex flex-col gap-2">
            <FieldRow label="Model key" hint="Provider-namespaced model identifier.">
              <div className="flex flex-col gap-2">
                <InlineInput
                  value={effectiveEmbedding}
                  onChange={setEmbeddingModel}
                  placeholder="e.g. openai:text-embedding-3-small"
                />
                <div className="flex items-center gap-2">
                  <SaveButton
                    onClick={handleSaveEmbedding}
                    isPending={embeddingsMutation.isPending}
                    saved={embeddingSaved}
                  />
                  <FieldError error={embeddingsMutation.error} />
                </div>
              </div>
            </FieldRow>
          </div>
        </section>
      )}

      {/* ── BYO adapters ─────────────────────────────────────────────────── */}
      {data && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
          <div className="flex items-center justify-between px-4 py-3 sm:px-5">
            <div>
              <h2 className="text-[14px] font-[600] text-t1">Adapters</h2>
              <p className="text-[12px] text-t3 mt-0.5">
                Your BYO provider connections. Secrets are write-only — stored encrypted,
                never returned.
              </p>
            </div>
            {!showAdapterForm && !editingAdapter && (
              <button
                type="button"
                onClick={() => setShowAdapterForm(true)}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 text-[12.5px] font-[500]",
                  "bg-surface-2 border border-line text-t2",
                  "hover:bg-surface-3 transition-colors duration-[--cb-dur]",
                )}
              >
                <Plus size={13} strokeWidth={2.5} aria-hidden="true" />
                Add
              </button>
            )}
          </div>

          <div className="px-4 py-4 sm:px-5 flex flex-col gap-3">
            {showAdapterForm && !editingAdapter && (
              <AdapterForm
                onCancel={() => setShowAdapterForm(false)}
                onSaved={() => setShowAdapterForm(false)}
              />
            )}

            {editingAdapter && (
              <AdapterForm
                initial={editingAdapter}
                onCancel={() => setEditingAdapter(null)}
                onSaved={() => setEditingAdapter(null)}
              />
            )}

            {byoAdapters.length === 0 && !showAdapterForm ? (
              <p className="text-[12.5px] text-t4 py-2">
                No adapters yet. Add one to use your own API keys.
              </p>
            ) : (
              byoAdapters.map((a) => (
                <AdapterRow
                  key={a.id}
                  adapter={a}
                  onEdit={(adapter) => {
                    setShowAdapterForm(false);
                    setEditingAdapter(adapter);
                  }}
                />
              ))
            )}
          </div>
        </section>
      )}

      {/* ── AI prompts ────────────────────────────────────────────────────── */}
      {prompts && prompts.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
          <div className="px-4 py-3 sm:px-5">
            <h2 className="text-[14px] font-[600] text-t1">Custom guidance</h2>
            <p className="text-[12px] text-t3 mt-0.5">
              Override Scout&apos;s built-in instructions for specific tasks. Leave blank to use
              the default.
            </p>
          </div>

          <div className="px-4 sm:px-5">
            {prompts.map((p) => (
              <PromptRow
                key={p.purpose}
                purpose={p.purpose}
                label={p.label}
                description={p.description}
                instructions={p.instructions}
                configured={p.configured}
              />
            ))}
          </div>
        </section>
      )}

      {/* Prompts error (non-blocking — show inline) */}
      {promptsError && (
        <p className="text-[12px] text-danger mb-4">
          Could not load AI prompts.
        </p>
      )}
    </div>
  );
};
