/**
 * AccountPage — Settings → Account at /settings/account.
 *
 * Fetches GET /api/app/account and renders editable fields:
 *   - Name (read-only: no direct field, shown for reference)
 *   - Email (read-only)
 *   - Language — PATCH /api/app/account/language
 *   - Compose preference — PATCH /api/app/account/compose_preference
 *   - Writing style — PATCH /api/app/account/writing_style
 *   - Password change — PATCH /api/app/account (requires current password)
 *
 * Each section saves independently with optimistic cache update.
 * Inline error display below the field on mutation failure.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useAccountQuery,
  useAccountLanguageMutation,
  useAccountComposeMutation,
  useAccountWritingStyleMutation,
  useAccountPasswordMutation,
} from "~/modules/settings/api";
import { cn } from "~/lib/utils";

// ── Field row helpers ─────────────────────────────────────────────────────────

interface FieldRowProps {
  label: string;
  hint?: string;
  children: React.ReactNode;
}

const FieldRow: FC<FieldRowProps> = ({ label, hint, children }) => (
  <div className="flex flex-col sm:flex-row sm:items-start gap-2 sm:gap-6 py-4 border-b border-line last:border-b-0">
    <div className="sm:w-[180px] shrink-0">
      <span className="text-[13px] font-[500] text-t1">{label}</span>
      {hint && <p className="text-[12px] text-t4 mt-0.5 leading-snug">{hint}</p>}
    </div>
    <div className="flex-1 min-w-0">{children}</div>
  </div>
);

// ── Inline text input ─────────────────────────────────────────────────────────

interface InlineInputProps {
  value: string;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  readOnly?: boolean;
}

const InlineInput: FC<InlineInputProps> = ({
  value,
  onChange,
  type = "text",
  placeholder,
  disabled,
  readOnly,
}) => (
  <input
    type={type}
    value={value}
    onChange={(e) => onChange(e.target.value)}
    placeholder={placeholder}
    disabled={disabled}
    readOnly={readOnly}
    className={cn(
      "w-full px-3 py-2 rounded-cb-1 text-[13px]",
      "bg-surface-3 border border-line",
      "text-t1 placeholder:text-t4",
      "focus:outline-none focus:ring-2 focus:ring-[--ring]",
      "disabled:opacity-40 disabled:cursor-not-allowed",
      readOnly && "text-t3 cursor-default select-all",
      "transition-colors duration-[--cb-dur]",
    )}
  />
);

// ── Save button ───────────────────────────────────────────────────────────────

interface SaveButtonProps {
  onClick: () => void;
  isPending: boolean;
  saved: boolean;
}

const SaveButton: FC<SaveButtonProps> = ({ onClick, isPending, saved }) => (
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
      "Save"
    )}
  </button>
);

// ── Error display ─────────────────────────────────────────────────────────────

const FieldError: FC<{ error: Error | null }> = ({ error }) => {
  if (!error) return null;
  const msg = error instanceof ApiError ? error.message : "Something went wrong";
  return (
    <p className="text-[12px] text-danger mt-1.5" role="alert">
      {msg}
    </p>
  );
};

// ── SUPPORTED_LOCALES ─────────────────────────────────────────────────────────

const LOCALE_OPTIONS = [
  { value: "en", label: "English" },
  { value: "pt", label: "Português (PT)" },
  { value: "es", label: "Español" },
  { value: "fr", label: "Français" },
];

const COMPOSE_OPTIONS = [
  { value: "chat", label: "Chat (default)" },
  { value: "classic", label: "Classic compose" },
];

// ── AccountPage ───────────────────────────────────────────────────────────────

export const AccountPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useAccountQuery();

  const langMutation     = useAccountLanguageMutation();
  const composeMutation  = useAccountComposeMutation();
  const styleMutation    = useAccountWritingStyleMutation();
  const passwordMutation = useAccountPasswordMutation();

  // Controlled state for each editable field
  const [locale,          setLocale]         = useState<string | null>(null);
  const [composeDefault,  setComposeDefault] = useState<string | null>(null);
  const [writingStyle,    setWritingStyle]   = useState<string | null>(null);
  const [savedField,      setSavedField]     = useState<string | null>(null);

  // Password change fields
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword,     setNewPassword]     = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  // Resolve current value: local draft wins, then server, then default
  const effectiveLocale   = locale          ?? data?.locale          ?? "en";
  const effectiveCompose  = composeDefault  ?? data?.compose_default ?? "chat";
  const effectiveStyle    = writingStyle    ?? data?.writing_style   ?? "";

  // Save handlers
  const handleSaveLocale = useCallback((): void => {
    langMutation.mutate(
      { locale: effectiveLocale },
      {
        onSuccess: () => {
          setLocale(null);
          setSavedField("locale");
          setTimeout(() => setSavedField(null), 2000);
        },
      },
    );
  }, [langMutation, effectiveLocale]);

  const handleSaveCompose = useCallback((): void => {
    composeMutation.mutate(
      { compose_default: effectiveCompose },
      {
        onSuccess: () => {
          setComposeDefault(null);
          setSavedField("compose");
          setTimeout(() => setSavedField(null), 2000);
        },
      },
    );
  }, [composeMutation, effectiveCompose]);

  const handleSaveStyle = useCallback((): void => {
    styleMutation.mutate(
      { writing_style: effectiveStyle },
      {
        onSuccess: () => {
          setWritingStyle(null);
          setSavedField("style");
          setTimeout(() => setSavedField(null), 2000);
        },
      },
    );
  }, [styleMutation, effectiveStyle]);

  const handleSavePassword = useCallback((): void => {
    passwordMutation.mutate(
      {
        current_password: currentPassword,
        password: newPassword,
        password_confirmation: confirmPassword,
      },
      {
        onSuccess: () => {
          setCurrentPassword("");
          setNewPassword("");
          setConfirmPassword("");
          setSavedField("password");
          setTimeout(() => setSavedField(null), 2000);
        },
      },
    );
  }, [passwordMutation, currentPassword, newPassword, confirmPassword]);

  // ── Loading / error states ─────────────────────────────────────────────────

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading account settings"
      >
        <div className="h-6 w-40 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2, 3].map((i) => (
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
        <Visor state="asleep" size={28} label="Error loading account" />
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
      <h1 className="text-[17px] font-[650] text-t1 mb-6">Account</h1>

      {/* Read-only identity */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <FieldRow label="Name">
          <InlineInput value={data.name ?? ""} onChange={() => void 0} readOnly />
        </FieldRow>
        <FieldRow label="Email">
          <InlineInput value={data.email} onChange={() => void 0} readOnly />
        </FieldRow>
        <FieldRow label="Role">
          <span className="text-[13px] text-t2 capitalize">{data.role}</span>
        </FieldRow>
      </section>

      {/* Language */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <FieldRow
          label="Language"
          hint="The language the interface is displayed in."
        >
          <div className="flex flex-col gap-2">
            <select
              value={effectiveLocale}
              onChange={(e) => setLocale(e.target.value)}
              className={cn(
                "w-full px-3 py-2 rounded-cb-1 text-[13px]",
                "bg-surface-3 border border-line text-t1",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label="Language"
            >
              {LOCALE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <div className="flex items-center gap-2">
              <SaveButton
                onClick={handleSaveLocale}
                isPending={langMutation.isPending}
                saved={savedField === "locale"}
              />
              <FieldError error={langMutation.error} />
            </div>
          </div>
        </FieldRow>
      </section>

      {/* Compose preference */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <FieldRow
          label="Compose mode"
          hint="How new emails open when you hit Reply or New."
        >
          <div className="flex flex-col gap-2">
            <select
              value={effectiveCompose}
              onChange={(e) => setComposeDefault(e.target.value)}
              className={cn(
                "w-full px-3 py-2 rounded-cb-1 text-[13px]",
                "bg-surface-3 border border-line text-t1",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label="Compose mode"
            >
              {COMPOSE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <div className="flex items-center gap-2">
              <SaveButton
                onClick={handleSaveCompose}
                isPending={composeMutation.isPending}
                saved={savedField === "compose"}
              />
              <FieldError error={composeMutation.error} />
            </div>
          </div>
        </FieldRow>
      </section>

      {/* Writing style */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <FieldRow
          label="Writing style"
          hint="Scout uses this when drafting emails on your behalf."
        >
          <div className="flex flex-col gap-2">
            <textarea
              value={effectiveStyle}
              onChange={(e) => setWritingStyle(e.target.value)}
              placeholder="E.g. Concise and professional; avoid bullet points; end with a question."
              rows={4}
              className={cn(
                "w-full px-3 py-2 rounded-cb-1 text-[13px] resize-y",
                "bg-surface-3 border border-line",
                "text-t1 placeholder:text-t4",
                "focus:outline-none focus:ring-2 focus:ring-[--ring]",
                "transition-colors duration-[--cb-dur]",
              )}
              aria-label="Writing style"
            />
            {data.writing_style_learned && (
              <p className="text-[11.5px] text-t4">
                Scout learned this from your sent mail.
                {data.writing_style_updated_at && (
                  <> Last updated: {new Date(data.writing_style_updated_at).toLocaleDateString()}.</>
                )}
              </p>
            )}
            <div className="flex items-center gap-2">
              <SaveButton
                onClick={handleSaveStyle}
                isPending={styleMutation.isPending}
                saved={savedField === "style"}
              />
              <FieldError error={styleMutation.error} />
            </div>
          </div>
        </FieldRow>
      </section>

      {/* Password change */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <div className="px-4 py-3 sm:px-5">
          <h2 className="text-[14px] font-[600] text-t1">Change password</h2>
        </div>
        <FieldRow label="Current password">
          <InlineInput
            type="password"
            value={currentPassword}
            onChange={setCurrentPassword}
            placeholder="Enter current password"
          />
        </FieldRow>
        <FieldRow label="New password">
          <InlineInput
            type="password"
            value={newPassword}
            onChange={setNewPassword}
            placeholder="At least 12 characters"
          />
        </FieldRow>
        <FieldRow label="Confirm password">
          <div className="flex flex-col gap-2">
            <InlineInput
              type="password"
              value={confirmPassword}
              onChange={setConfirmPassword}
              placeholder="Repeat new password"
            />
            <div className="flex items-center gap-2">
              <SaveButton
                onClick={handleSavePassword}
                isPending={passwordMutation.isPending}
                saved={savedField === "password"}
              />
              <FieldError error={passwordMutation.error} />
            </div>
          </div>
        </FieldRow>
      </section>
    </div>
  );
};
