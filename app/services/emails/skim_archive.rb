# frozen_string_literal: true

module Emails
  # Archives a set of emails chosen in Skim Mode. Security-scoped: only emails the
  # user can actually read are touched, so a forged id list can't reach another
  # account's mail. Delegates the real work to the existing Tools::BulkArchive.
  class SkimArchive
    # Pure input sanitiser (coerce, drop blanks/zeros/negatives, dedup).
    def self.sanitize_ids(raw)
      Array(raw).map { |value| value.to_i }.reject { |id| id <= 0 }.uniq
    end

    def initialize(user, raw_ids)
      @user = user
      @ids = self.class.sanitize_ids(raw_ids)
    end

    # Returns the number of emails actually archived.
    def call
      return 0 if @ids.empty?

      owned = EmailMessage
        .where(id: @ids, email_account: @user.readable_email_accounts)
        .pluck(:id)
      return 0 if owned.empty?

      Tools::BulkArchive.call("email_ids" => owned)
      owned.size
    end
  end
end
