import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Avatar,
  Banner,
  Button,
  EmptyState,
  InlineConfirm,
  List,
  ListRow,
  Segmented,
  SkeletonList,
  Text,
  TextField,
} from "~/lib/ui";

import {
  allPages,
  useDecideSignupRequest,
  useSignupRequests,
  type SignupRequest,
  type SignupRequestStatus,
} from "../../api/admin";

type Filter = SignupRequestStatus | "all";

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

/** Admin → Signups → Requests: people waiting to create their first workspace. */
export function SignupRequests() {
  const { t } = useTranslation("workspaces");
  const [filter, setFilter] = useState<Filter>("all");
  const requests = useSignupRequests(filter);
  const rows = allPages(requests.data, (page) => page.requests);

  return (
    <>
      <Segmented
        label={t("signups.requests.filter.label")}
        value={filter}
        onChange={setFilter}
        options={[
          { value: "all", label: t("signups.requests.filter.all") },
          { value: "pending", label: t("signups.requests.filter.pending") },
          { value: "approved", label: t("signups.requests.filter.approved") },
          { value: "rejected", label: t("signups.requests.filter.rejected") },
        ]}
      />
      {requests.isPending ? (
        <div role="status" aria-label={t("signups.requests.loading")}>
          <SkeletonList rows={3} />
        </div>
      ) : requests.isError ? (
        <EmptyState
          title={t("signups.loadError.title")}
          visor="thinking"
          visorSize="lg"
          action={
            <Button
              onClick={() => {
                void requests.refetch();
              }}
            >
              {t("signups.loadError.retry")}
            </Button>
          }
        >
          {t("signups.loadError.lead")}
        </EmptyState>
      ) : rows.length === 0 ? (
        <EmptyState
          title={t(`signups.requests.empty.${filter}.title`)}
          visor="asleep"
          visorSize="lg"
        >
          {t(`signups.requests.empty.${filter}.lead`)}
        </EmptyState>
      ) : (
        <>
          <List label={t("signups.requests.list")}>
            {rows.map((request) => (
              <RequestRow key={request.user_id} request={request} />
            ))}
          </List>
          {requests.hasNextPage ? (
            <div>
              <Button
                variant="secondary"
                disabled={requests.isFetchingNextPage}
                onClick={() => {
                  void requests.fetchNextPage();
                }}
              >
                {t("signups.more")}
              </Button>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}

type Pending = "approve" | "reject";

function RequestRow({ request }: { request: SignupRequest }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const decide = useDecideSignupRequest();
  const [confirming, setConfirming] = useState<Pending | null>(null);
  const [reason, setReason] = useState("");
  const [failed, setFailed] = useState(false);
  const name = request.display_name ?? request.email ?? request.user_id;

  const meta = [
    request.email === name ? null : request.email,
    t("signups.requests.asked", { date: formatDate(request.requested_at) }),
    request.decided_at === null || request.decided_at === undefined
      ? null
      : t("signups.requests.decided", { date: formatDate(request.decided_at) }),
  ]
    .filter((part) => part !== null && part !== undefined)
    .join(" · ");

  const close = () => {
    setConfirming(null);
    setReason("");
  };
  const confirm = (decision: Pending) => {
    setFailed(false);
    decide.mutate(
      { userId: request.user_id, decision, reason },
      {
        onSuccess: close,
        onError: () => {
          close();
          setFailed(true);
        },
      },
    );
  };

  return (
    <ListRow
      leading={<Avatar name={name} />}
      title={name}
      badge={t(`signups.requests.status.${request.status}`)}
      meta={meta}
      muted={request.status === "rejected"}
      trailing={
        <>
          {request.status === "approved" ? null : (
            <Button
              variant={request.status === "pending" ? "primary" : "secondary"}
              aria-label={t("signups.requests.approveLabel", { name })}
              aria-expanded={confirming === "approve"}
              onClick={() => {
                setFailed(false);
                setConfirming("approve");
              }}
            >
              {t("signups.requests.approve")}
            </Button>
          )}
          {request.status === "rejected" ? null : (
            <Button
              variant="ghost"
              aria-label={t("signups.requests.rejectLabel", { name })}
              aria-expanded={confirming === "reject"}
              onClick={() => {
                setFailed(false);
                setConfirming("reject");
              }}
            >
              {t("signups.requests.reject")}
            </Button>
          )}
        </>
      }
    >
      {request.reason === null || request.reason === undefined ? null : (
        <Text tone="secondary" size="sm">
          {t("signups.requests.reasonLine", { reason: request.reason })}
        </Text>
      )}
      {confirming === "reject" ? (
        <TextField
          label={t("signups.requests.reason")}
          hint={t("signups.requests.reasonHint")}
          maxLength={500}
          autoComplete="off"
          value={reason}
          onChange={(event) => {
            setReason(event.target.value);
          }}
        />
      ) : null}
      {confirming === null ? null : (
        <InlineConfirm
          confirmLabel={t(`signups.requests.${confirming}`)}
          cancelLabel={t("signups.cancel")}
          busy={decide.isPending}
          onCancel={close}
          onConfirm={() => {
            confirm(confirming);
          }}
        >
          {t(`signups.requests.confirm.${confirming}`, { name })}
        </InlineConfirm>
      )}
      {failed ? <Banner tone="danger" role="alert" title={t("signups.errors.generic")} /> : null}
    </ListRow>
  );
}
