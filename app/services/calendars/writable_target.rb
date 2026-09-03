# frozen_string_literal: true

module Calendars
  # Picks the calendar a suggested item (a pending reminder, a kept focus block)
  # should become a real event on: the calendar of the SAME mailbox the item came
  # from — so the event pushes back to that Google/Zoho account, sharing the
  # source's OAuth grant — else the user's primary writable calendar. Returns nil
  # when the user has no writable calendar (the caller then keeps the item local).
  #
  # Shared by Reminders::Confirm and Time::FocusKeeper so the "which calendar?"
  # decision lives in one place.
  class WritableTarget
    def self.for(user:, source_email_account: nil)
      new(user, source_email_account).call
    end

    def initialize(user, source_email_account)
      @user = user
      @source_email_account = source_email_account
    end

    def call
      source_account_calendar || primary_writable_calendar
    end

    private

    def source_account_calendar
      account = source_calendar_account
      return nil unless account && @user&.writable_calendar_accounts&.exists?(account.id)

      account.calendars.where(is_writable: true, syncing: true).order(is_primary: :desc).first
    end

    def primary_writable_calendar
      return nil unless @user

      Calendar.where(calendar_account: @user.writable_calendar_accounts, is_writable: true, syncing: true)
              .order(is_primary: :desc).first
    end

    # The CalendarAccount provisioned from the source mailbox — matched on
    # email_address + provider (Calendars::AccountProvisioner pairs them that way).
    def source_calendar_account
      ea = @source_email_account
      return nil unless ea && CalendarAccount.providers.key?(ea.provider)

      CalendarAccount.find_by(workspace_id: ea.workspace_id, email_address: ea.email_address, provider: ea.provider)
    end
  end
end
