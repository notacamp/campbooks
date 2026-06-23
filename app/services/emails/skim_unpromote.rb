# frozen_string_literal: true

module Emails
  # Removes a set of emails from Skim's "Priority" lane (clears pinned_at).
  # Security-scoped like Emails::SkimArchive.
  class SkimUnpromote
    def initialize(user, raw_ids)
      @user = user
      @ids = Emails::SkimArchive.sanitize_ids(raw_ids)
    end

    # Returns the number of emails actually unpinned.
    def call
      return 0 if @ids.empty?

      EmailMessage
        .where(id: @ids, email_account: @user.readable_email_accounts)
        .update_all(pinned_at: nil, updated_at: Time.current)
    end
  end
end
