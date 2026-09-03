# frozen_string_literal: true

module Now
  # The one place the Now surface defines "Scout's actions in the window": the
  # workspace's SYSTEM events (actor_id IS NULL — automation, not a person),
  # accessible to the user, since a cutoff. Both the ledger (counts) and the log
  # (rows) read from here so their window and their system-only filter never drift.
  module SystemEvents
    def self.scope(user, since:)
      return Event.none unless user&.workspace

      user.workspace.events
          .accessible_to(user)
          .where(actor_id: nil)
          .where(occurred_at: since..)
    end
  end
end
