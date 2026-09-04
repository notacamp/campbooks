# frozen_string_literal: true

module Home
  # The zero-state of a user's inbox, shared by the Home and Now deck's cleared/
  # connect states so the two never diverge. One of:
  #   :syncing      — a live inbox whose first scan is still running
  #   :caught_up    — a live inbox; the queue is genuinely clear
  #   :disconnected — connected before, but every inbox is now inactive
  #   :none         — brand-new; never connected an inbox
  #
  # It answers "what kind of inbox does this user have right now", NOT "is the feed
  # empty" — the caller decides whether to show it (Home only in its empty branch;
  # Now whenever the deck has no cards). Distinguishing :disconnected from :none
  # matters: a user whose inboxes all went inactive must not be told to "connect
  # your inbox" as if brand new.
  class InboxState
    def self.for(user)
      new(user).state
    end

    def initialize(user)
      @user = user
    end

    def state
      accounts = @user&.workspace&.email_accounts
      if accounts&.active&.exists?
        Onboarding::FirstSyncStatus.new(@user).stage? ? :syncing : :caught_up
      elsif accounts&.exists?
        :disconnected
      else
        :none
      end
    end
  end
end
