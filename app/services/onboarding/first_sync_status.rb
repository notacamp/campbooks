# frozen_string_literal: true

module Onboarding
  # The first inbox sync as the user experiences it: one aggregate state plus
  # live counters, driving the "Scout is reading your inbox" stage on home
  # (initial render and the polling JSON both read from here).
  #
  #   waiting   — account connected, no scan has picked it up yet (queue lag)
  #   scanning  — a first scan is in flight
  #   error     — every attempt so far failed and nothing was ingested
  #   empty     — first scan finished and the mailbox had nothing
  #   done      — first scan finished with mail in
  #
  # The stage only exists for a genuinely fresh mailbox: once any scan has
  # completed, home renders normally forever after (stage? is false).
  class FirstSyncStatus
    def initialize(user)
      @user = user
    end

    # Show the full-screen stage? True from "just connected" until the first
    # completed scan, so a reload mid-scan comes back to the stage.
    def stage?
      accounts.any? && !any_scan_completed?
    end

    def state
      return :done if any_scan_completed? && found.positive?
      return :empty if any_scan_completed?
      return :scanning if scan_logs.running.exists?
      return :error if scan_logs.failed.exists?

      :waiting
    end

    def found
      @found ||= messages.count
    end

    def sorted
      @sorted ||= messages.where.not(category: nil).count
    end

    def needs_you
      @needs_you ||= messages.where(ai_priority: EmailMessage.ai_priorities[:high]).count
    end

    def as_json(*)
      { state: state, found: found, sorted: sorted, needs_you: needs_you }
    end

    private

    attr_reader :user

    def accounts
      @accounts ||= user.email_accounts.active
    end

    def scan_logs
      EmailScanLog.where(email_account: accounts)
    end

    def any_scan_completed?
      return @any_scan_completed if defined?(@any_scan_completed)
      @any_scan_completed = scan_logs.completed.exists?
    end

    def messages
      EmailMessage.where(email_account: accounts)
    end
  end
end
