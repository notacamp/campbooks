import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Banner,
  Button,
  CopyField,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  SkeletonList,
  Text,
  TextField,
} from "~/lib/ui";

import {
  allPages,
  useMintSignupCode,
  useRevokeSignupCode,
  useSignupCodes,
  type MintedSignupCode,
  type SignupCode,
} from "../../api/admin";

const MAX_LABEL = 100;
const MAX_USES = 100_000;

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

/** The end of the local day `day` (`YYYY-MM-DD`), as an instant; null when not a day. */
export function endOfDay(day: string): string | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(day);
  if (match === null) {
    return null;
  }
  const [, year = "", month = "", date = ""] = match;
  const end = new Date(Number(year), Number(month) - 1, Number(date), 23, 59, 59);
  return Number.isNaN(end.getTime()) ? null : end.toISOString();
}

/** Admin → Signups → Codes: mint single-use codes, see their use, revoke them. */
export function SignupCodes() {
  const { t } = useTranslation("workspaces");
  const codes = useSignupCodes();
  const rows = allPages(codes.data, (page) => page.codes);
  const listTitle = useId();

  return (
    <>
      <MintForm />
      <section className="flex flex-col gap-3" aria-labelledby={listTitle}>
        <Heading level={2} size="section" id={listTitle}>
          {t("signups.codes.listTitle")}
        </Heading>
        {codes.isPending ? (
          <div role="status" aria-label={t("signups.codes.loading")}>
            <SkeletonList rows={2} />
          </div>
        ) : codes.isError ? (
          <Banner
            tone="danger"
            title={t("signups.loadError.title")}
            action={
              <Button
                onClick={() => {
                  void codes.refetch();
                }}
              >
                {t("signups.loadError.retry")}
              </Button>
            }
          >
            {t("signups.loadError.lead")}
          </Banner>
        ) : rows.length === 0 ? (
          <EmptyState title={t("signups.codes.empty.title")} visor="asleep" visorSize="lg">
            {t("signups.codes.empty.lead")}
          </EmptyState>
        ) : (
          <>
            <List label={t("signups.codes.list")}>
              {rows.map((code) => (
                <CodeRow key={code.id} code={code} />
              ))}
            </List>
            {codes.hasNextPage ? (
              <div>
                <Button
                  variant="secondary"
                  disabled={codes.isFetchingNextPage}
                  onClick={() => {
                    void codes.fetchNextPage();
                  }}
                >
                  {t("signups.more")}
                </Button>
              </div>
            ) : null}
          </>
        )}
      </section>
    </>
  );
}

function MintForm() {
  const { t } = useTranslation("workspaces");
  const mint = useMintSignupCode();
  const [label, setLabel] = useState("");
  const [uses, setUses] = useState("1");
  const [expires, setExpires] = useState("");
  const [errors, setErrors] = useState<{ label?: string; uses?: string; expires?: string }>({});
  const [failed, setFailed] = useState(false);
  const [issued, setIssued] = useState<MintedSignupCode | undefined>();
  const titleId = useId();

  const submit = () => {
    setIssued(undefined);
    setFailed(false);
    const trimmed = label.trim();
    const count = Number(uses);
    const expiry = expires === "" ? undefined : endOfDay(expires);
    const next = {
      ...(trimmed === "" || trimmed.length > MAX_LABEL
        ? { label: t("signups.codes.mint.errors.label") }
        : {}),
      ...(!Number.isInteger(count) || count < 1 || count > MAX_USES
        ? { uses: t("signups.codes.mint.errors.uses") }
        : {}),
      ...(expiry === null || (expiry !== undefined && new Date(expiry) <= new Date())
        ? { expires: t("signups.codes.mint.errors.expires") }
        : {}),
    };
    setErrors(next);
    if (Object.keys(next).length > 0) {
      return;
    }
    mint.mutate(
      {
        label: trimmed,
        max_uses: count,
        ...(expiry === undefined || expiry === null ? {} : { expires_at: expiry }),
      },
      {
        onSuccess: (minted) => {
          setLabel("");
          setUses("1");
          setExpires("");
          setIssued(minted);
        },
        onError: (failure) => {
          if (errorStatus(failure) === 400) {
            setErrors({ label: t("signups.codes.mint.errors.label") });
          } else {
            setFailed(true);
          }
        },
      },
    );
  };

  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("signups.codes.mint.title")}
      </Heading>
      <Text tone="tertiary" size="sm">
        {t("signups.codes.mint.lead")}
      </Text>
      <form
        className="flex flex-col gap-3 sm:flex-row sm:items-start"
        aria-labelledby={titleId}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="flex-1">
          <TextField
            label={t("signups.codes.mint.label")}
            placeholder={t("signups.codes.mint.placeholder")}
            autoComplete="off"
            value={label}
            error={errors.label}
            onChange={(event) => {
              setLabel(event.target.value);
            }}
          />
        </div>
        <TextField
          label={t("signups.codes.mint.uses")}
          type="number"
          inputMode="numeric"
          min={1}
          max={MAX_USES}
          value={uses}
          error={errors.uses}
          onChange={(event) => {
            setUses(event.target.value);
          }}
        />
        <TextField
          label={t("signups.codes.mint.expires")}
          type="date"
          value={expires}
          error={errors.expires}
          onChange={(event) => {
            setExpires(event.target.value);
          }}
        />
        <div className="flex sm:pt-6">
          <Button type="submit" variant="primary" disabled={mint.isPending}>
            {mint.isPending ? t("signups.codes.mint.minting") : t("signups.codes.mint.submit")}
          </Button>
        </div>
      </form>
      {failed ? <Banner tone="danger" role="alert" title={t("signups.codes.mint.failed")} /> : null}
      {issued === undefined ? null : (
        <>
          <Banner
            icon="check"
            role="status"
            title={t("signups.codes.minted.title", { label: issued.entry.label })}
          >
            {t("signups.codes.minted.lead")}
          </Banner>
          <CopyField
            label={t("signups.codes.minted.field")}
            value={issued.code}
            copyLabel={t("signups.codes.minted.copy")}
            copiedLabel={t("signups.codes.minted.copied")}
            failedLabel={t("signups.codes.minted.copyFailed")}
          />
          <div>
            <Button
              variant="ghost"
              onClick={() => {
                setIssued(undefined);
              }}
            >
              {t("signups.codes.minted.done")}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}

function CodeRow({ code }: { code: SignupCode }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const revoke = useRevokeSignupCode();
  const [confirming, setConfirming] = useState(false);
  const [failed, setFailed] = useState(false);

  const meta = [
    t("signups.codes.usage", { uses: code.uses, max: code.max_uses }),
    code.expires_at === null || code.expires_at === undefined
      ? t("signups.codes.noExpiry")
      : t("signups.codes.expiresOn", { date: formatDate(code.expires_at) }),
    t("signups.codes.mintedOn", { date: formatDate(code.created_at) }),
  ].join(" · ");

  return (
    <ListRow
      title={code.label}
      badge={t(`signups.codes.state.${code.state}`)}
      meta={meta}
      muted={code.state !== "active"}
      trailing={
        code.state === "revoked" ? null : (
          <Button
            variant="ghost"
            aria-label={t("signups.codes.revokeLabel", { label: code.label })}
            aria-expanded={confirming}
            onClick={() => {
              setFailed(false);
              setConfirming(true);
            }}
          >
            {t("signups.codes.revoke")}
          </Button>
        )
      }
    >
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("signups.codes.revoke")}
          cancelLabel={t("signups.cancel")}
          busy={revoke.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            revoke.mutate(code.id, {
              onSuccess: () => {
                setConfirming(false);
              },
              onError: () => {
                setConfirming(false);
                setFailed(true);
              },
            });
          }}
        >
          {t("signups.codes.confirmRevoke", { label: code.label })}
        </InlineConfirm>
      ) : null}
      {failed ? <Banner tone="danger" role="alert" title={t("signups.errors.generic")} /> : null}
    </ListRow>
  );
}
