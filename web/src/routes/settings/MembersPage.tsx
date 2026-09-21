/**
 * MembersPage — Settings → Members at /settings/members.
 *
 * Fetches GET /api/app/settings/members and renders:
 *   - Member list with role selector (admin-only mutation via PATCH /:id)
 *   - Pending invitations with resend + cancel; approve for admins
 *   - Invite form: POST /api/app/settings/invitations
 *
 * Admin-gated controls mirror WorkspacePage — non-admins see read-only.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, UserPlus, Check, X, RefreshCw, ShieldCheck } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import { useMe } from "~/lib/api";
import {
  useMembersQuery,
  useMemberRoleMutation,
  useCreateInvitationMutation,
  useCancelInvitationMutation,
  useResendInvitationMutation,
  useApproveInvitationMutation,
} from "~/modules/settings/api";
import type { MemberData, InvitationData } from "~/modules/settings/types";
import { cn } from "~/lib/utils";

// ── Helpers ───────────────────────────────────────────────────────────────────

const FieldError: FC<{ error: Error | null }> = ({ error }) => {
  if (!error) return null;
  const msg = error instanceof ApiError ? error.message : "Something went wrong";
  return (
    <p className="text-[12px] text-danger mt-1.5" role="alert">
      {msg}
    </p>
  );
};

const ROLE_OPTIONS = [
  { value: "member", label: "Member" },
  { value: "admin",  label: "Admin" },
];

const INVITATION_STATUS_LABELS: Record<string, string> = {
  pending:   "Pending",
  accepted:  "Accepted",
  expired:   "Expired",
  cancelled: "Cancelled",
};

// ── MemberRow ─────────────────────────────────────────────────────────────────

interface MemberRowProps {
  member: MemberData;
  currentUserId: number | undefined;
  isAdmin: boolean;
  onRoleChange: (id: number, role: string) => void;
  isPending: boolean;
}

const MemberRow: FC<MemberRowProps> = ({
  member,
  currentUserId,
  isAdmin,
  onRoleChange,
  isPending,
}) => {
  const isSelf = member.id === currentUserId;

  return (
    <div className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 py-4 border-b border-line last:border-b-0">
      <div className="flex items-center gap-3 flex-1 min-w-0">
        <div
          className={cn(
            "w-8 h-8 rounded-full shrink-0 flex items-center justify-center",
            "bg-surface-3 border border-line text-[12px] font-[600] text-t2 uppercase",
          )}
          aria-hidden="true"
        >
          {(member.name ?? member.email).charAt(0)}
        </div>
        <div className="min-w-0">
          <p className="text-[13px] font-[500] text-t1 truncate">
            {member.name ?? <span className="text-t3">No name</span>}
            {isSelf && (
              <span className="ml-1.5 text-[11px] text-t4 font-normal">
                (you)
              </span>
            )}
          </p>
          <p className="text-[12px] text-t3 truncate">{member.email}</p>
        </div>
      </div>

      <div className="shrink-0 pl-11 sm:pl-0">
        {isAdmin && !isSelf ? (
          <select
            value={member.role}
            onChange={(e) => onRoleChange(member.id, e.target.value)}
            disabled={isPending}
            aria-label={`Role for ${member.name ?? member.email}`}
            className={cn(
              "px-2.5 py-1.5 rounded-cb-1 text-[12.5px]",
              "bg-surface-3 border border-line text-t1",
              "focus:outline-none focus:ring-2 focus:ring-[--ring]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
              "transition-colors duration-[--cb-dur]",
            )}
          >
            {ROLE_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        ) : (
          <span
            className={cn(
              "inline-flex items-center gap-1 text-[12px] px-2.5 py-1 rounded-cb-1 border capitalize",
              member.role === "admin"
                ? "bg-surface-2 border-line text-t1 font-[500]"
                : "bg-surface-3 border-line text-t3",
            )}
          >
            {member.role === "admin" && (
              <ShieldCheck size={11} strokeWidth={2} aria-hidden="true" />
            )}
            {member.role}
          </span>
        )}
      </div>
    </div>
  );
};

// ── InvitationRow ─────────────────────────────────────────────────────────────

interface InvitationRowProps {
  invitation: InvitationData;
  currentUserId: number | undefined;
  isAdmin: boolean;
  onResend: (id: number) => void;
  onCancel: (id: number) => void;
  onApprove: (id: number) => void;
  isPending: boolean;
}

const InvitationRow: FC<InvitationRowProps> = ({
  invitation: inv,
  currentUserId,
  isAdmin,
  onResend,
  onCancel,
  onApprove,
  isPending,
}) => {
  const canManage = isAdmin || inv.invited_by?.id === currentUserId;
  const statusLabel =
    INVITATION_STATUS_LABELS[inv.status] ?? inv.status;

  return (
    <div className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 py-4 border-b border-line last:border-b-0">
      <div className="flex-1 min-w-0">
        <p className="text-[13px] text-t1 truncate">{inv.email}</p>
        <div className="flex flex-wrap items-center gap-2 mt-0.5">
          <span
            className={cn(
              "text-[11px] px-1.5 py-0.5 rounded-cb-1 border capitalize",
              inv.status === "pending"
                ? "bg-surface-2 border-line text-t2"
                : "bg-surface-3 border-line text-t4",
            )}
          >
            {statusLabel}
          </span>
          {!inv.admin_approved && isAdmin && (
            <span className="text-[11px] text-danger">Pending approval</span>
          )}
          {inv.invited_by && (
            <span className="text-[11px] text-t4">
              by {inv.invited_by.name ?? inv.invited_by.email}
            </span>
          )}
        </div>
      </div>

      {canManage && inv.status === "pending" && (
        <div className="flex items-center gap-1.5 shrink-0 pl-0 sm:pl-0">
          {isAdmin && !inv.admin_approved && (
            <button
              type="button"
              aria-label="Approve invitation"
              onClick={() => onApprove(inv.id)}
              disabled={isPending}
              className={cn(
                "text-[12px] text-t2 px-2.5 py-1 rounded-cb-1 border border-line",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                "disabled:opacity-40 flex items-center gap-1",
              )}
            >
              <Check size={12} strokeWidth={2.5} />
              Approve
            </button>
          )}
          <button
            type="button"
            aria-label="Resend invitation"
            onClick={() => onResend(inv.id)}
            disabled={isPending}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-t1 hover:bg-surface-3 border border-transparent",
              "transition-colors duration-[--cb-dur] disabled:opacity-40",
            )}
          >
            <RefreshCw size={13} strokeWidth={2} />
          </button>
          <button
            type="button"
            aria-label="Cancel invitation"
            onClick={() => onCancel(inv.id)}
            disabled={isPending}
            className={cn(
              "p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-surface-3 border border-transparent",
              "transition-colors duration-[--cb-dur] disabled:opacity-40",
            )}
          >
            <X size={13} strokeWidth={2} />
          </button>
        </div>
      )}
    </div>
  );
};

// ── Invite form ───────────────────────────────────────────────────────────────

interface InviteFormProps {
  onSuccess: () => void;
}

const InviteForm: FC<InviteFormProps> = ({ onSuccess }) => {
  const [email, setEmail] = useState("");
  const [sent,  setSent]  = useState(false);
  const mutation = useCreateInvitationMutation();

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!email.trim()) return;
      mutation.mutate(
        { email: email.trim() },
        {
          onSuccess: () => {
            setEmail("");
            setSent(true);
            setTimeout(() => setSent(false), 3000);
            onSuccess();
          },
        },
      );
    },
    [mutation, email, onSuccess],
  );

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-2">
      <div className="flex flex-col sm:flex-row gap-2">
        <input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="colleague@example.com"
          required
          disabled={mutation.isPending}
          className={cn(
            "flex-1 px-3 py-2 rounded-cb-1 text-[13px]",
            "bg-surface-3 border border-line",
            "text-t1 placeholder:text-t4",
            "focus:outline-none focus:ring-2 focus:ring-[--ring]",
            "disabled:opacity-40 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
          aria-label="Email address to invite"
        />
        <button
          type="submit"
          disabled={mutation.isPending || !email.trim()}
          className={cn(
            "shrink-0 flex items-center gap-1.5 px-3 py-2 rounded-cb-1 text-[12.5px] font-[500]",
            sent
              ? "bg-surface-1 border border-line text-t4"
              : "bg-[--primary] text-[--primary-foreground]",
            "disabled:opacity-60 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          {sent ? (
            <>
              <Check size={12} strokeWidth={2.5} aria-hidden="true" />
              Sent
            </>
          ) : mutation.isPending ? (
            "Sending…"
          ) : (
            <>
              <UserPlus size={13} strokeWidth={2} aria-hidden="true" />
              Invite
            </>
          )}
        </button>
      </div>
      <FieldError error={mutation.error} />
    </form>
  );
};

// ── MembersPage ───────────────────────────────────────────────────────────────

export const MembersPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useMembersQuery();
  const { data: me } = useMe();

  const roleMutation     = useMemberRoleMutation();
  const cancelMutation   = useCancelInvitationMutation();
  const resendMutation   = useResendInvitationMutation();
  const approveMutation  = useApproveInvitationMutation();

  const isAdmin = me?.user?.role === "admin";
  const currentUserId = me?.user?.id;

  const isMutating =
    roleMutation.isPending ||
    cancelMutation.isPending ||
    resendMutation.isPending ||
    approveMutation.isPending;

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading members"
      >
        <div className="h-6 w-36 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-16 bg-surface-1 rounded-cb-1" />
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
        <Visor state="asleep" size={28} label="Error loading members" />
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

  const pendingInvitations = data.invitations.filter(
    (i) => i.status === "pending",
  );
  const otherInvitations = data.invitations.filter(
    (i) => i.status !== "pending",
  );

  return (
    <div data-testid="settings.page.root" className="max-w-2xl">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-[17px] font-[650] text-t1">Members</h1>
        {!isAdmin && (
          <span className="text-[12px] text-t4 bg-surface-1 border border-line px-2 py-1 rounded-cb-1">
            View only — admins only
          </span>
        )}
      </div>

      {/* Member list */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
        <div className="px-4 py-3 sm:px-5 border-b border-line">
          <h2 className="text-[14px] font-[600] text-t1">
            Team members
            <span className="ml-2 text-[12px] text-t4 font-normal">
              ({data.members.length})
            </span>
          </h2>
        </div>
        <div className="px-4 sm:px-5">
          {data.members.map((member) => (
            <MemberRow
              key={member.id}
              member={member}
              currentUserId={currentUserId}
              isAdmin={isAdmin}
              onRoleChange={(id, role) => roleMutation.mutate({ id, role })}
              isPending={isMutating}
            />
          ))}
        </div>
        {roleMutation.error && (
          <div className="px-4 sm:px-5 pb-3">
            <FieldError error={roleMutation.error} />
          </div>
        )}
      </section>

      {/* Invite form (all members can invite) */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
        <div className="px-4 py-3 sm:px-5 border-b border-line">
          <h2 className="text-[14px] font-[600] text-t1">Invite someone</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Send an invitation to their email address.
          </p>
        </div>
        <div className="px-4 py-4 sm:px-5">
          <InviteForm onSuccess={() => void 0} />
        </div>
      </section>

      {/* Pending invitations */}
      {pendingInvitations.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line">
            <h2 className="text-[14px] font-[600] text-t1">
              Pending invitations
              <span className="ml-2 text-[12px] text-t4 font-normal">
                ({pendingInvitations.length})
              </span>
            </h2>
          </div>
          <div className="px-4 sm:px-5">
            {pendingInvitations.map((inv) => (
              <InvitationRow
                key={inv.id}
                invitation={inv}
                currentUserId={currentUserId}
                isAdmin={isAdmin}
                onResend={(id) => resendMutation.mutate(id)}
                onCancel={(id) => cancelMutation.mutate(id)}
                onApprove={(id) => approveMutation.mutate(id)}
                isPending={isMutating}
              />
            ))}
          </div>
        </section>
      )}

      {/* Past invitations (read-only) */}
      {otherInvitations.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line">
            <h2 className="text-[14px] font-[600] text-t1">Past invitations</h2>
          </div>
          <div className="px-4 sm:px-5">
            {otherInvitations.map((inv) => (
              <InvitationRow
                key={inv.id}
                invitation={inv}
                currentUserId={currentUserId}
                isAdmin={isAdmin}
                onResend={(id) => resendMutation.mutate(id)}
                onCancel={(id) => cancelMutation.mutate(id)}
                onApprove={(id) => approveMutation.mutate(id)}
                isPending={isMutating}
              />
            ))}
          </div>
        </section>
      )}
    </div>
  );
};
