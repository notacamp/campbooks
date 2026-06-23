module InboxSettings
  # Connected accounts + sync status panel. Connect/re-auth/disconnect go through
  # the existing EmailAccountsController (OAuth / token removal); this panel just
  # presents them and can kick off a scan.
  class AccountsController < BaseController
    def show
      load_accounts
    end

    def scan_now
      # Manual scans force a full re-baseline — the user explicitly asked to resync,
      # so walk every folder and re-establish the delta cursor rather than just pull
      # the cheap incremental changes the every-minute poll already covers.
      EmailScanJob.perform_later(nil, "full")
      render turbo_stream: notify_stream(t(".started"))
    end

    private

    def load_accounts
      @accounts = Current.user.email_accounts.order(:email_address)
      @recent_scans = EmailScanLog.where(email_account_id: @accounts.pluck(:id))
                                  .order(created_at: :desc)
                                  .limit(8)
                                  .includes(:email_account)
    end
  end
end
