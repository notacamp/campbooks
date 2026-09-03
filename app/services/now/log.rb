# frozen_string_literal: true

module Now
  # Scout's log under the deck: the most recent SYSTEM events (Scout/automation,
  # actor_id IS NULL) accessible to the user in the window, newest first. Backs
  # Campbooks::Now::LogRow. Shares the window + system-only filter with Now::Ledger
  # (via Now::SystemEvents) so the log and the ledger can never disagree.
  class Log
    LIMIT = 20

    def initialize(user, since: 24.hours.ago)
      @user = user
      @since = since
    end

    # The rows to render (capped, newest first, subjects preloaded to keep the
    # Open/Undo links from N+1-ing).
    def events
      @events ||= scope.recent.limit(LIMIT).includes(:subject).to_a
    end

    # Total actions in the window — the "last 24 hours · N" header count. Counts
    # everything Scout did, not just the capped rows shown.
    def count
      @count ||= scope.count
    end

    def any?
      events.any?
    end

    private

    def scope
      Now::SystemEvents.scope(@user, since: @since)
    end
  end
end
