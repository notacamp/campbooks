# frozen_string_literal: true

module Emails
  # Promotes a set of emails into Skim's "Priority" lane (sets pinned_at).
  # Security-scoped exactly like Emails::SkimArchive — only mail the user can read
  # is touched, so a forged id list can't reach another account's inbox.
  class SkimPromote
    def initialize(user, raw_ids)
      @user = user
      @ids = Emails::SkimArchive.sanitize_ids(raw_ids)
    end

    # Returns the number of emails actually pinned.
    def call
      return 0 if @ids.empty?

      EmailMessage
        .where(id: @ids, email_account: @user.readable_email_accounts)
        .update_all(pinned_at: Time.current, updated_at: Time.current)
    end
  end
end
