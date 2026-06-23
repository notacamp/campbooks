# frozen_string_literal: true

module Emails
  # The single source of truth for "which emails Skim should consider" for a user.
  # Used by both SkimController (show/tray) and the real-time broadcaster so they
  # always agree.
  #
  # Bounded by a real TIME WINDOW (plus anything pinned, regardless of age) rather
  # than a blanket "most-recent N" — so the count reflects actual recent volume and
  # drops as mail is cleared (the old `.limit(200)` refilled from the backlog on
  # every load, so it was perma-pinned at 200). Excludes mail already addressed in
  # Skim (`skimmed_at`) so the feed never re-shows what you've handled; archived
  # mail is already excluded because it leaves the inbox folder.
  class SkimScope
    # How far back the recent rings reach. Pinned mail ignores this.
    WINDOW = 14.days
    # Safety ceiling so a pathological inbox can't bloat the DOM. A normal window
    # sits well under this, so it does not reintroduce the perma-cap behaviour.
    MAX = 500

    SELECT = %i[
      id from_address subject summary ai_summary received_at email_account_id provider_folder_id
      read ai_priority category email_thread_id pinned_at skimmed_at contact_id
    ].freeze

    def self.for(user)
      new(user).relation
    end

    def initialize(user)
      @user = user
    end

    def relation
      accounts = @user.readable_email_accounts.to_a
      return EmailMessage.none if accounts.empty?

      scope = EmailMessage.where(email_account: accounts).where(skimmed_at: nil)
      scope = Emails::InboxFolders.constrain(scope, accounts)

      scope
        .where("email_messages.received_at >= :since OR email_messages.pinned_at IS NOT NULL", since: WINDOW.ago)
        .includes(:contact)
        .select(*SELECT)
        .order(received_at: :desc)
        .limit(MAX)
    end
  end
end
