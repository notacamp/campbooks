# frozen_string_literal: true

# Enforces the workspace's "linked email accounts" plan limit at connect time.
# Shared by the three mail OAuth callbacks (gmail/zoho/microsoft).
#
# IMPORTANT: call this ONLY on the genuine-create path. Reconnecting/reactivating
# an already-known mailbox takes the find_by → update! branch and must NOT count
# against the cap, so the guard never runs for it.
module EmailAccountCapGuard
  extend ActiveSupport::Concern

  private

  # True (and redirects) when the current workspace can't link another mailbox on
  # its plan; false (and does nothing) when it may.
  def email_account_cap_reached?
    return false if current_entitlements.allow?(:email_accounts) == :ok

    if native_oauth?
      redirect_to_native(flow: "connect", status: "limit_reached")
    else
      redirect_to account_link_failure_path,
                  error: t("entitlements.blocked.email_accounts_cap",
                           limit: current_entitlements.limit(:email_accounts),
                           plan: current_entitlements.plan_name)
    end
    true
  end
end
