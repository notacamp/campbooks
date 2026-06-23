# frozen_string_literal: true

module Emails
  # Marks a set of emails as addressed in Skim ("Keep" — handled but kept in the
  # inbox), so the feed does not surface them again (sets skimmed_at). Archived
  # mail already leaves the inbox folder; this covers the kept-in-place case.
  # Security-scoped like Emails::SkimArchive.
  class SkimDismiss
    def initialize(user, raw_ids)
      @user = user
      @ids = Emails::SkimArchive.sanitize_ids(raw_ids)
    end

    # Returns the number of emails marked addressed.
    def call
      return 0 if @ids.empty?

      EmailMessage
        .where(id: @ids, email_account: @user.readable_email_accounts)
        .update_all(skimmed_at: Time.current, updated_at: Time.current)
    end
  end
end
