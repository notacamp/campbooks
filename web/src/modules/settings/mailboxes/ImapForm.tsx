/**
 * ImapForm — inline form to connect a generic IMAP/SMTP mailbox.
 * Calls POST /api/app/imap_accounts; shows inline server-reported errors.
 * Collapsible behind a "Connect any IMAP account" expand trigger.
 *
 * Presentation layer — HTML elements and raw copy are intentional here.
 */
/* eslint-disable react/forbid-elements -- ImapForm is a design-layer component */
/* eslint-disable react/forbid-component-props -- className on lib/ui icons is intentional */
/* eslint-disable react/jsx-no-literals -- UI copy lives at the surface layer */
import { type FC, type ChangeEvent, type FormEvent, useState } from "react";
import { ChevronDown, ChevronUp, Loader2 } from "lucide-react";
import { cn } from "~/lib/utils";
import { ApiError } from "~/lib/api";
import { useImapConnectMutation } from "./hooks";
import type { ImapConnectParams } from "./types";

// ── ImapForm ──────────────────────────────────────────────────────────────────

interface ImapFormProps {
  onSuccess?: () => void;
}

interface FormState {
  email_address: string;
  password: string;
  imap_host: string;
  imap_port: string;
  imap_ssl: boolean;
  smtp_host: string;
  smtp_port: string;
  smtp_ssl: boolean;
}

interface FormErrors {
  email_address?: string;
  password?: string;
  imap_host?: string;
  imap_port?: string;
  smtp_host?: string;
  smtp_port?: string;
}

const INITIAL: FormState = {
  email_address: "",
  password: "",
  imap_host: "",
  imap_port: "993",
  imap_ssl: true,
  smtp_host: "",
  smtp_port: "587",
  smtp_ssl: true,
};

export const ImapForm: FC<ImapFormProps> = ({ onSuccess }) => {
  const [open, setOpen] = useState(false);
  const [fields, setFields] = useState<FormState>(INITIAL);
  const [fieldErrors, setFieldErrors] = useState<FormErrors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const mutation = useImapConnectMutation();

  const setField = (key: keyof FormState) => (e: ChangeEvent<HTMLInputElement>) => {
    const value = e.target.type === "checkbox" ? e.target.checked : e.target.value;
    setFields((prev) => ({ ...prev, [key]: value }));
    if (key in fieldErrors) {
      setFieldErrors((prev) => ({ ...prev, [key]: undefined }));
    }
  };

  const validate = (): boolean => {
    const errors: FormErrors = {};
    if (!fields.email_address.match(/\S+@\S+\.\S+/)) {
      errors.email_address = "Valid email required";
    }
    if (!fields.password) {
      errors.password = "Required";
    }
    if (!fields.imap_host) {
      errors.imap_host = "Required";
    }
    const imapPort = Number(fields.imap_port);
    if (!fields.imap_port || isNaN(imapPort) || imapPort < 1 || imapPort > 65535) {
      errors.imap_port = "1–65535";
    }
    if (!fields.smtp_host) {
      errors.smtp_host = "Required";
    }
    const smtpPort = Number(fields.smtp_port);
    if (!fields.smtp_port || isNaN(smtpPort) || smtpPort < 1 || smtpPort > 65535) {
      errors.smtp_port = "1–65535";
    }
    setFieldErrors(errors);
    return Object.keys(errors).length === 0;
  };

  const handleSubmit = async (e: FormEvent<HTMLFormElement>): Promise<void> => {
    e.preventDefault();
    setServerError(null);
    if (!validate()) return;

    const params: ImapConnectParams = {
      email_address: fields.email_address.trim().toLowerCase(),
      password: fields.password,
      imap_host: fields.imap_host.trim(),
      imap_port: Number(fields.imap_port),
      imap_ssl: fields.imap_ssl,
      smtp_host: fields.smtp_host.trim(),
      smtp_port: Number(fields.smtp_port),
      smtp_ssl: fields.smtp_ssl,
    };

    try {
      await mutation.mutateAsync(params);
      setFields(INITIAL);
      setOpen(false);
      onSuccess?.();
    } catch (err) {
      if (err instanceof ApiError) {
        setServerError(err.message);
      } else {
        setServerError("Something went wrong — please try again");
      }
    }
  };

  return (
    <div
      className="rounded-cb-2 bg-surface-1 border border-line overflow-hidden"
      data-testid="mailboxes.imap-form"
    >
      {/* Trigger row */}
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "flex items-center justify-between w-full px-4 py-3",
          "text-[13px] font-medium text-t2 text-left",
          "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          "focus-visible:ring-2 focus-visible:ring-cb-accent focus-visible:ring-inset focus-visible:outline-none",
        )}
        aria-expanded={open}
        data-testid="mailboxes.imap-form.toggle"
      >
        <span>Connect any IMAP account</span>
        {open ? (
          <ChevronUp size={14} className="text-t3" aria-hidden="true" />
        ) : (
          <ChevronDown size={14} className="text-t3" aria-hidden="true" />
        )}
      </button>

      {/* Collapsible form */}
      {open && (
        <form
          onSubmit={(e) => { void handleSubmit(e); }}
          className="px-4 pb-4 pt-1 border-t border-line grid gap-3"
          noValidate
          aria-label="IMAP mailbox connect form"
        >
          {serverError && (
            <p
              className="text-[12px] text-cb-danger bg-cb-danger/10 rounded-cb-1 px-3 py-2"
              role="alert"
              data-testid="mailboxes.imap-form.server-error"
            >
              {serverError}
            </p>
          )}

          {/* Email + password */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <Field label="Email address" error={fieldErrors.email_address}>
              <input
                type="email"
                value={fields.email_address}
                onChange={setField("email_address")}
                autoComplete="email"
                placeholder="you@example.com"
                className={inputClass(!!fieldErrors.email_address)}
                data-testid="mailboxes.imap-form.email"
              />
            </Field>

            <Field label="Password" error={fieldErrors.password}>
              <input
                type="password"
                value={fields.password}
                onChange={setField("password")}
                autoComplete="current-password"
                placeholder="App password or IMAP password"
                className={inputClass(!!fieldErrors.password)}
                data-testid="mailboxes.imap-form.password"
              />
            </Field>
          </div>

          {/* IMAP */}
          <p className="text-[11px] font-medium text-t3 uppercase tracking-wide">
            IMAP (incoming)
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <Field
              label="Host"
              error={fieldErrors.imap_host}
              className="sm:col-span-2"
            >
              <input
                type="text"
                value={fields.imap_host}
                onChange={setField("imap_host")}
                placeholder="imap.example.com"
                className={inputClass(!!fieldErrors.imap_host)}
                data-testid="mailboxes.imap-form.imap-host"
              />
            </Field>
            <Field label="Port" error={fieldErrors.imap_port}>
              <input
                type="number"
                value={fields.imap_port}
                onChange={setField("imap_port")}
                placeholder="993"
                className={inputClass(!!fieldErrors.imap_port)}
                data-testid="mailboxes.imap-form.imap-port"
              />
            </Field>
          </div>

          {/* SMTP */}
          <p className="text-[11px] font-medium text-t3 uppercase tracking-wide">
            SMTP (outgoing)
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <Field
              label="Host"
              error={fieldErrors.smtp_host}
              className="sm:col-span-2"
            >
              <input
                type="text"
                value={fields.smtp_host}
                onChange={setField("smtp_host")}
                placeholder="smtp.example.com"
                className={inputClass(!!fieldErrors.smtp_host)}
                data-testid="mailboxes.imap-form.smtp-host"
              />
            </Field>
            <Field label="Port" error={fieldErrors.smtp_port}>
              <input
                type="number"
                value={fields.smtp_port}
                onChange={setField("smtp_port")}
                placeholder="587"
                className={inputClass(!!fieldErrors.smtp_port)}
                data-testid="mailboxes.imap-form.smtp-port"
              />
            </Field>
          </div>

          {/* Submit */}
          <button
            type="submit"
            disabled={mutation.isPending}
            className={cn(
              "flex items-center justify-center gap-2 mt-1",
              "w-full sm:w-auto sm:ml-auto",
              "px-4 py-2 rounded-cb-1 text-[13px] font-medium",
              "bg-cb-accent text-white",
              "hover:bg-cb-accent/90 active:scale-[0.97]",
              "transition-all duration-[--cb-dur]",
              "disabled:opacity-60 disabled:cursor-not-allowed",
              "focus-visible:ring-2 focus-visible:ring-cb-accent focus-visible:ring-offset-2 focus-visible:outline-none",
            )}
            data-testid="mailboxes.imap-form.submit"
          >
            {mutation.isPending && (
              <Loader2 size={13} className="animate-spin" aria-hidden="true" />
            )}
            Connect mailbox
          </button>
        </form>
      )}
    </div>
  );
};

// ── Helpers ───────────────────────────────────────────────────────────────────

const inputClass = (hasError: boolean): string =>
  cn(
    "w-full px-3 py-2 rounded-cb-1 text-[13px] text-t1",
    "bg-surface-2 border border-line",
    "placeholder:text-t4",
    "focus:outline-none focus:ring-2 focus:ring-cb-accent focus:border-transparent",
    "transition-colors duration-[--cb-dur]",
    hasError && "border-cb-danger focus:ring-cb-danger",
  );

interface FieldProps {
  label: string;
  error?: string;
  className?: string;
  children: React.ReactNode;
}

const Field: FC<FieldProps> = ({ label, error, className, children }) => (
  <div className={cn("flex flex-col gap-1", className)}>
    <label className="text-[11.5px] font-medium text-t3">{label}</label>
    {children}
    {error && (
      <p className="text-[11px] text-cb-danger" role="alert">
        {error}
      </p>
    )}
  </div>
);
