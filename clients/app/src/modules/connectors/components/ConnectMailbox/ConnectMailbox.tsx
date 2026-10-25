import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import {
  Banner,
  Button,
  ButtonLink,
  Checkbox,
  Dialog,
  Disclosure,
  Select,
  Steps,
  Text,
  TextField,
} from "~/lib/ui";

import {
  connectImap,
  remember,
  useCheckAddress,
  useConnection,
  useDiscoverImap,
  type Connection,
  type DiscoveredImap,
  type MailServer,
  type MailServers,
} from "../../api/queries";
import type { MailboxConsent } from "../../consent";
import { guideFor, type Guide } from "../../guides";
import { connectError, knownMailProvider, type ConnectError } from "../../model";

export interface ConnectMailboxProps {
  workspaceId: string;
  /** The front door, for the mailboxes connected with their provider's sign-in. */
  consent: MailboxConsent;
  open: boolean;
  onClose: () => void;
}

/** Where the dialog is. */
type Step =
  | { kind: "address" }
  | { kind: "native"; provider: "google" | "zoho" }
  | { kind: "outlook" }
  | { kind: "password"; found: DiscoveredImap }
  | { kind: "fallback" }
  | { kind: "check"; connection: string };

/** The servers as the Advanced fields hold them (ports as typed). */
interface ServerFields {
  host: string;
  port: string;
  security: MailServer["security"];
}

const EMPTY_IMAP: ServerFields = { host: "", port: "993", security: "tls" };
const EMPTY_SMTP: ServerFields = { host: "", port: "465", security: "tls" };

const fields = (found: MailServer | null | undefined, empty: ServerFields): ServerFields =>
  found === null || found === undefined
    ? empty
    : { host: found.host, port: String(found.port), security: found.security };

/** `fields` as a server, or `undefined` when a host is missing or a port is not one. */
function server({ host, port, security }: ServerFields): MailServer | undefined {
  const number = Number(port);
  return host.trim() === "" || !Number.isInteger(number) || number < 1 || number > 65_535
    ? undefined
    : { host: host.trim(), port: number, security };
}

/**
 * Add a mailbox (campbooks-cloud#314): the address first, then the way it
 * connects. Gmail and Zoho go to their own sign-in (the front door), Outlook
 * is coming soon, and any other mailbox connects with an app password: the
 * provider's steps, one password field, and Advanced servers prefilled from
 * discovery. An address the login does not prove is then checked by
 * Campbooks, which this dialog follows. The password never stays in the
 * page's state once it is sent.
 */
export function ConnectMailbox({ workspaceId, consent, open, onClose }: ConnectMailboxProps) {
  const { t } = useTranslation("connectors");
  const [step, setStep] = useState<Step>({ kind: "address" });
  const [address, setAddress] = useState("");
  const [addressError, setAddressError] = useState<string | undefined>();
  const [stepUp, setStepUp] = useState(false);
  const [failed, setFailed] = useState<ConnectError | undefined>();
  const discover = useDiscoverImap(workspaceId);

  const reset = () => {
    setStep({ kind: "address" });
    setAddress("");
    setAddressError(undefined);
    setStepUp(false);
    setFailed(undefined);
    discover.reset();
  };
  const close = () => {
    reset();
    onClose();
  };
  const back = () => {
    setStep({ kind: "address" });
    setStepUp(false);
    setFailed(undefined);
  };

  const find = (event: FormEvent) => {
    event.preventDefault();
    const typed = address.trim();
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(typed)) {
      setAddressError(t("add.address.invalid"));
      return;
    }
    setAddressError(undefined);
    setStepUp(false);
    setFailed(undefined);
    discover.mutate(typed, {
      onSuccess: (found) => {
        if (
          found.kind === "use_provider" &&
          (found.provider === "google" || found.provider === "zoho")
        ) {
          setStep({ kind: "native", provider: found.provider });
        } else if (found.kind === "use_provider" || found.kind === "not_supported_yet") {
          setStep({ kind: "outlook" });
        } else {
          setStep({ kind: "password", found });
        }
      },
      onError: (error) => {
        const code = connectError(error);
        if (code === "step_up") {
          setStepUp(true);
        } else if (code === "provider_not_configured") {
          setStep({ kind: "fallback" });
        } else if (code === "imap_address_invalid") {
          setAddressError(t("add.address.invalid"));
        } else {
          setFailed(code);
        }
      },
    });
  };

  const title = (() => {
    switch (step.kind) {
      case "address": {
        return t("add.title");
      }
      case "native": {
        return t("add.native.title", { provider: t(`provider.${step.provider}`) });
      }
      case "outlook": {
        return t("add.outlook.title");
      }
      case "password": {
        return t("add.password.title", { address });
      }
      case "fallback": {
        return t("add.fallback.title");
      }
      case "check": {
        return t("add.check.title", { address });
      }
    }
  })();

  return (
    <Dialog open={open} onClose={close} title={title} visor={null}>
      <div className="flex flex-col gap-4">
        {stepUp ? <StepUp consent={consent} /> : null}
        {step.kind === "address" ? (
          <AddressStep
            address={address}
            error={addressError}
            failed={failed}
            busy={discover.isPending}
            onAddress={setAddress}
            onSubmit={find}
            onCancel={close}
          />
        ) : null}
        {step.kind === "native" ? (
          <NativeStep provider={step.provider} address={address} consent={consent} onBack={back} />
        ) : null}
        {step.kind === "outlook" ? <OutlookStep address={address} onBack={back} /> : null}
        {step.kind === "fallback" ? <FallbackStep consent={consent} onBack={back} /> : null}
        {step.kind === "password" ? (
          <PasswordStep
            workspaceId={workspaceId}
            address={address}
            found={step.found}
            onStepUp={() => {
              setStepUp(true);
            }}
            onBack={back}
            onConnected={(connection) => {
              setStepUp(false);
              if (connection.address_verified) {
                close();
              } else {
                setStep({ kind: "check", connection: connection.id });
              }
            }}
          />
        ) : null}
        {step.kind === "check" ? (
          <CheckStep
            workspaceId={workspaceId}
            connectionId={step.connection}
            address={address}
            onDone={close}
          />
        ) : null}
      </div>
    </Dialog>
  );
}

/** "Sign in again first": the screen's own step-up, inside the dialog. */
export function StepUp({ consent }: { consent: MailboxConsent }) {
  const { t } = useTranslation("connectors");
  return (
    <Banner
      role="alert"
      icon="key"
      title={t("stepUp.title")}
      action={
        <Button variant="secondary" onClick={consent.signInAgain}>
          {t("stepUp.action")}
        </Button>
      }
    >
      {t("stepUp.lead")}
    </Banner>
  );
}

/** A footer row: the step's buttons, the main one last. */
function Actions({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap justify-end gap-2">{children}</div>;
}

function AddressStep({
  address,
  error,
  failed,
  busy,
  onAddress,
  onSubmit,
  onCancel,
}: {
  address: string;
  error: string | undefined;
  failed: ConnectError | undefined;
  busy: boolean;
  onAddress: (address: string) => void;
  onSubmit: (event: FormEvent) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation("connectors");
  return (
    <form className="flex flex-col gap-4" onSubmit={onSubmit} noValidate>
      <TextField
        label={t("add.address.label")}
        hint={t("add.address.hint")}
        error={error}
        type="email"
        inputMode="email"
        autoComplete="email"
        autoCapitalize="none"
        spellCheck={false}
        data-autofocus=""
        value={address}
        onChange={(event) => {
          onAddress(event.target.value);
        }}
      />
      {failed === undefined ? null : (
        <Banner tone="danger" role="alert" title={t(`add.errors.${errorKey(failed)}`)} />
      )}
      {busy ? (
        <Text tone="secondary" size="sm">
          <span role="status">{t("add.finding")}</span>
        </Text>
      ) : null}
      <Actions>
        <Button variant="ghost" onClick={onCancel}>
          {t("add.cancel")}
        </Button>
        <Button type="submit" variant="primary" keys="enter" disabled={busy}>
          {t("add.continue")}
        </Button>
      </Actions>
    </form>
  );
}

/** A connect error's message key (`step_up` is said by its banner). */
function errorKey(code: ConnectError): Exclude<ConnectError, "step_up"> {
  return code === "step_up" ? "unknown" : code;
}

function NativeStep({
  provider,
  address,
  consent,
  onBack,
}: {
  provider: "google" | "zoho";
  address: string;
  consent: MailboxConsent;
  onBack: () => void;
}) {
  const { t } = useTranslation("connectors");
  const [signIn, setSignIn] = useState(true);
  const name = t(`provider.${provider}`);
  const offered = consent.providers?.includes(provider) ?? false;
  return (
    <div className="flex flex-col gap-4">
      <Text>{t("add.native.lead", { address, provider: name })}</Text>
      <ConsentRefusal consent={consent} />
      {offered ? (
        <Checkbox
          label={t("connect.signIn")}
          hint={t("connect.signInHint")}
          checked={signIn}
          onChange={(event) => {
            setSignIn(event.currentTarget.checked);
          }}
        />
      ) : (
        <Text tone="secondary">{t("add.native.notHere", { provider: name })}</Text>
      )}
      <Actions>
        <Button variant="ghost" onClick={onBack}>
          {t("add.back")}
        </Button>
        {offered ? (
          <Button
            variant="primary"
            icon="arrow-right"
            keys="enter"
            data-autofocus=""
            disabled={consent.starting}
            onClick={() => {
              consent.start({ purpose: "connect", provider, signIn });
            }}
          >
            {t("connect.provider", { provider: name })}
          </Button>
        ) : null}
      </Actions>
    </div>
  );
}

/** Why the front door refused to start, said in the dialog. */
function ConsentRefusal({ consent }: { consent: MailboxConsent }) {
  const { t } = useTranslation("connectors");
  if (consent.refusal === undefined) {
    return null;
  }
  if (consent.refusal === "step_up") {
    return <StepUp consent={consent} />;
  }
  return <Banner tone="danger" role="alert" title={t(`refusal.${consent.refusal}`)} />;
}

function OutlookStep({ address, onBack }: { address: string; onBack: () => void }) {
  const { t } = useTranslation("connectors");
  return (
    <div className="flex flex-col gap-4">
      <Text>{t("add.outlook.lead", { address })}</Text>
      <Actions>
        <Button variant="primary" data-autofocus="" onClick={onBack}>
          {t("add.back")}
        </Button>
      </Actions>
    </div>
  );
}

/** IMAP is off on this instance: the providers it connects, as before. */
function FallbackStep({ consent, onBack }: { consent: MailboxConsent; onBack: () => void }) {
  const { t } = useTranslation("connectors");
  const [signIn, setSignIn] = useState(true);
  const providers = (consent.providers ?? []).filter(knownMailProvider);
  return (
    <div className="flex flex-col gap-4">
      <Text>{providers.length > 0 ? t("add.fallback.lead") : t("connect.none")}</Text>
      <ConsentRefusal consent={consent} />
      {providers.length > 0 ? (
        <>
          <Checkbox
            label={t("connect.signIn")}
            hint={t("connect.signInHint")}
            checked={signIn}
            onChange={(event) => {
              setSignIn(event.currentTarget.checked);
            }}
          />
          <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
            {providers.map((provider) => (
              <Button
                key={provider}
                variant="secondary"
                icon="plus"
                disabled={consent.starting}
                onClick={() => {
                  consent.start({ purpose: "connect", provider, signIn });
                }}
              >
                {t("connect.provider", { provider: t(`provider.${provider}`) })}
              </Button>
            ))}
          </div>
        </>
      ) : null}
      <Actions>
        <Button variant="ghost" onClick={onBack}>
          {t("add.back")}
        </Button>
      </Actions>
    </div>
  );
}

/** Each guide's steps, in order (their message keys). */
const GUIDE_STEPS = {
  yahoo: ["add.guides.yahoo.s1", "add.guides.yahoo.s2", "add.guides.yahoo.s3"],
  aol: ["add.guides.aol.s1", "add.guides.aol.s2", "add.guides.aol.s3"],
  icloud: ["add.guides.icloud.s1", "add.guides.icloud.s2", "add.guides.icloud.s3"],
  fastmail: ["add.guides.fastmail.s1", "add.guides.fastmail.s2", "add.guides.fastmail.s3"],
  gmx: ["add.guides.gmx.s1", "add.guides.gmx.s2", "add.guides.gmx.s3", "add.guides.gmx.s4"],
  webde: [
    "add.guides.webde.s1",
    "add.guides.webde.s2",
    "add.guides.webde.s3",
    "add.guides.webde.s4",
  ],
  yandex: [
    "add.guides.yandex.s1",
    "add.guides.yandex.s2",
    "add.guides.yandex.s3",
    "add.guides.yandex.s4",
  ],
  custom: ["add.guides.custom.s1", "add.guides.custom.s2"],
} as const satisfies Record<Guide["id"], readonly string[]>;

function GuideSteps({ guide }: { guide: Guide }) {
  const { t } = useTranslation("connectors");
  const name = t(`add.guides.${guide.id}.name`);
  return (
    <div className="flex flex-col gap-2">
      <Steps
        label={t("add.guide.label", { provider: name })}
        steps={GUIDE_STEPS[guide.id].map((key) => t(key))}
      />
      {guide.help === undefined && guide.imap === undefined ? null : (
        <div className="flex flex-col gap-1 sm:flex-row sm:flex-wrap sm:gap-4">
          {guide.imap === undefined ? null : (
            <ButtonLink
              variant="link"
              icon="arrow-right"
              href={guide.imap}
              target="_blank"
              rel="noopener noreferrer"
            >
              {t("add.guide.imap", { provider: name })}
            </ButtonLink>
          )}
          {guide.help === undefined ? null : (
            <ButtonLink
              variant="link"
              icon="arrow-right"
              href={guide.help}
              target="_blank"
              rel="noopener noreferrer"
            >
              {t("add.guide.help", { provider: name })}
            </ButtonLink>
          )}
        </div>
      )}
    </div>
  );
}

function PasswordStep({
  workspaceId,
  address,
  found,
  onStepUp,
  onBack,
  onConnected,
}: {
  workspaceId: string;
  address: string;
  found: DiscoveredImap;
  onStepUp: () => void;
  onBack: () => void;
  onConnected: (connection: Connection) => void;
}) {
  const { t } = useTranslation("connectors");
  const queryClient = useQueryClient();
  const formId = useId();
  const passwordRef = useRef<HTMLInputElement>(null);
  const guide = guideFor(address, found.provider_hint);
  const appPassword = found.password === "app_password";
  const manual = found.kind === "not_found";
  const [password, setPassword] = useState("");
  const [username, setUsername] = useState(found.username_hint ?? address.trim());
  const [imap, setImap] = useState(() => fields(found.imap, EMPTY_IMAP));
  const [smtp, setSmtp] = useState(() => fields(found.smtp, EMPTY_SMTP));
  const [edited, setEdited] = useState(manual);
  const [advanced, setAdvanced] = useState(manual);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState<ConnectError | undefined>();
  const [missing, setMissing] = useState<"password" | "servers" | undefined>();

  // The password field is the step's question. Its value goes with the
  // step (Back, Esc, connected): it lives in this component's state only.
  useEffect(() => {
    passwordRef.current?.focus();
  }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (password === "") {
      setMissing("password");
      return;
    }
    const servers: MailServers | undefined = (() => {
      const imapServer = server(imap);
      const smtpServer = server(smtp);
      return imapServer === undefined || smtpServer === undefined
        ? undefined
        : { imap: imapServer, smtp: smtpServer };
    })();
    if (edited && servers === undefined) {
      setMissing("servers");
      setAdvanced(true);
      return;
    }
    setMissing(undefined);
    setFailed(undefined);
    // Out of the page's state the moment it is sent.
    const sent = password;
    setPassword("");
    setBusy(true);
    try {
      const connection = await connectImap(workspaceId, {
        address: address.trim(),
        password: sent,
        username:
          username.trim() === (found.username_hint ?? address.trim()) ? undefined : username,
        servers: edited ? servers : undefined,
      });
      remember(queryClient, workspaceId, connection);
      onConnected(connection);
    } catch (error) {
      const code = connectError(error);
      if (code === "step_up") {
        onStepUp();
      } else {
        setFailed(code);
        if (code === "imap_servers_required" || code === "host_refused") {
          setAdvanced(true);
        }
      }
      passwordRef.current?.focus();
    } finally {
      setBusy(false);
    }
  };

  const errorText =
    failed === undefined
      ? undefined
      : failed === "imap_login_refused" && appPassword
        ? t("add.errors.imap_login_refused_app", { provider: t(`add.guides.${guide.id}.name`) })
        : t(`add.errors.${errorKey(failed)}`);

  const securityOptions = [
    { value: "tls" as const, label: t("add.advanced.tls") },
    { value: "starttls" as const, label: t("add.advanced.starttls") },
  ];
  const serverFields = (
    which: "imap" | "smtp",
    value: ServerFields,
    onChange: (value: ServerFields) => void,
  ) => (
    <div className="flex flex-col gap-3" role="group" aria-labelledby={`${formId}-${which}`}>
      <Text as="span" weight="medium" id={`${formId}-${which}`}>
        {t(`add.advanced.${which}`)}
      </Text>
      <TextField
        label={t("add.advanced.host")}
        autoComplete="off"
        autoCapitalize="none"
        spellCheck={false}
        value={value.host}
        onChange={(event) => {
          setEdited(true);
          onChange({ ...value, host: event.target.value });
        }}
      />
      <div className="flex flex-col gap-3 sm:flex-row">
        <div className="sm:w-1/3">
          <TextField
            label={t("add.advanced.port")}
            inputMode="numeric"
            autoComplete="off"
            value={value.port}
            onChange={(event) => {
              setEdited(true);
              onChange({ ...value, port: event.target.value });
            }}
          />
        </div>
        <div className="sm:flex-1">
          <Select
            label={t("add.advanced.security")}
            options={securityOptions}
            value={value.security}
            onChange={(security) => {
              setEdited(true);
              onChange({ ...value, security });
            }}
          />
        </div>
      </div>
    </div>
  );

  return (
    <form
      id={formId}
      className="flex flex-col gap-4"
      noValidate
      onSubmit={(event) => {
        void submit(event);
      }}
    >
      <Text>
        {appPassword
          ? t("add.password.appLead", { provider: t(`add.guides.${guide.id}.name`) })
          : t("add.password.lead")}
      </Text>
      <GuideSteps guide={guide} />
      <TextField
        ref={passwordRef}
        label={appPassword ? t("add.password.appLabel") : t("add.password.label")}
        hint={t("add.password.hint")}
        error={missing === "password" ? t("add.password.required") : undefined}
        type="password"
        autoComplete="new-password"
        autoCapitalize="none"
        spellCheck={false}
        value={password}
        onChange={(event) => {
          setPassword(event.target.value);
        }}
      />
      {errorText === undefined ? null : <Banner tone="danger" role="alert" title={errorText} />}
      <Disclosure
        key={advanced ? "open" : "closed"}
        defaultOpen={advanced}
        summary={t("add.advanced.summary")}
      >
        <div className="flex flex-col gap-4">
          <TextField
            label={t("add.advanced.username")}
            hint={t("add.advanced.usernameHint")}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            value={username}
            onChange={(event) => {
              setUsername(event.target.value);
            }}
          />
          {serverFields("imap", imap, setImap)}
          {serverFields("smtp", smtp, setSmtp)}
          {missing === "servers" ? (
            <Banner tone="danger" role="alert" title={t("add.advanced.invalid")} />
          ) : null}
        </div>
      </Disclosure>
      <Actions>
        <Button variant="ghost" onClick={onBack} disabled={busy}>
          {t("add.back")}
        </Button>
        <Button type="submit" variant="primary" keys="enter" disabled={busy}>
          {busy ? t("add.password.connecting") : t("add.password.connect")}
        </Button>
      </Actions>
    </form>
  );
}

/** The address check of a mailbox just connected, followed until it ends. */
function CheckStep({
  workspaceId,
  connectionId,
  address,
  onDone,
}: {
  workspaceId: string;
  connectionId: string;
  address: string;
  onDone: () => void;
}) {
  const { t } = useTranslation("connectors");
  const connection = useConnection(workspaceId, connectionId);
  const again = useCheckAddress(workspaceId);
  const verified = connection.data?.address_verified === true;
  const checking = !verified && (connection.data?.address_check ?? "checking") === "checking";

  return (
    <div className="flex flex-col gap-4">
      <Text tone="secondary">{t("add.connected", { address })}</Text>
      <div role="status" className="flex flex-col gap-2">
        {verified ? (
          <>
            <Text weight="semibold">{t("add.check.verified")}</Text>
            <Text>{t("add.check.verifiedLead", { address })}</Text>
          </>
        ) : checking ? (
          <>
            <Text weight="semibold">{t("add.check.checking")}</Text>
            <Text>{t("add.check.checkingLead", { address })}</Text>
          </>
        ) : (
          <>
            <Text weight="semibold">{t("add.check.failed")}</Text>
            <Text>{t("add.check.failedLead", { address })}</Text>
          </>
        )}
      </div>
      {again.isError ? (
        <Banner tone="danger" role="alert" title={t(retryFailure(again.error))} />
      ) : null}
      <Actions>
        {!verified && !checking ? (
          <Button
            variant="secondary"
            disabled={again.isPending}
            onClick={() => {
              again.mutate(connectionId);
            }}
          >
            {t("add.check.retry")}
          </Button>
        ) : null}
        <Button variant="primary" keys="enter" data-autofocus="" onClick={onDone}>
          {t("add.check.done")}
        </Button>
      </Actions>
    </div>
  );
}

/** Why asking for the check again failed, as a message key. */
function retryFailure(error: unknown): "add.check.retryLimited" | "add.check.retryFailed" {
  return errorStatus(error) === 429 || problemCode(error) === "rate_limited"
    ? "add.check.retryLimited"
    : "add.check.retryFailed";
}
