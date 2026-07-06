# Shared by EmailAccount and CalendarAccount: turning an account's background sync
# off *with a recorded reason* when a provider service it depends on isn't
# available (a Google login with no Gmail mailbox, or no Google Calendar). Without
# this the scan jobs re-hit the doomed API every minute — 400/403 forever — and
# fill the health dashboard with noise the user can't act on.
#
# The reason is surfaced to the user (settings badge + the disconnect
# notification) so "reconnect" isn't the only story when reconnecting can't help.
module ProviderSyncDeactivation
  extend ActiveSupport::Concern

  # Machine reasons stored in `deactivation_reason`; each has a matching
  # `accounts.deactivation_reasons.<reason>` translation. NULL means a plain
  # disconnect (token revoked / user removed), which stays generic.
  DEACTIVATION_REASONS = %w[mail_service_unavailable calendar_service_unavailable].freeze

  included do
    # A reactivated account must never keep a stale "service unavailable" note.
    # Clearing whenever active is true also means a manual reactivation resets it —
    # and if the service is still gone, the next scan re-records it.
    before_save :clear_deactivation_reason_when_active
  end

  # Idempotently deactivate because a depended-on provider service is unavailable.
  # A no-op when already inactive, so the every-minute scan can't re-fire the
  # disconnect notification (or churn the row) each cycle.
  def deactivate_for!(reason)
    return unless active?
    update!(active: false, deactivation_reason: reason.to_s)
  end

  # Deactivated specifically because a service was unavailable (vs a plain
  # disconnect, which records no reason).
  def deactivated_for_service?
    !active? && deactivation_reason.present?
  end

  # Localized human explanation, or nil for a plain disconnect / unknown reason.
  def deactivation_reason_label
    return nil if deactivation_reason.blank?
    I18n.t("accounts.deactivation_reasons.#{deactivation_reason}", default: nil)
  end

  private

  def clear_deactivation_reason_when_active
    self.deactivation_reason = nil if active?
  end
end
