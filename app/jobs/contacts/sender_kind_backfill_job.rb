# frozen_string_literal: true

module Contacts
  # Classifies every not-yet-taught contact in a workspace with Contacts::SenderKind,
  # so the People place has a person/service verdict for the whole directory without
  # waiting for each sender's next message. Idempotent (re-running just re-derives
  # the heuristic), skips rows the user taught, and batches so a large mailbox
  # doesn't load every contact at once.
  #
  # Enqueued once per workspace from the `contacts:sender_kind_backfill` rake task
  # and, debounced, from PeopleController#index when a workspace still has
  # unclassified contacts with mail. Mirrors Feed::RefreshJob.enqueue_for.
  class SenderKindBackfillJob < ApplicationJob
    queue_as :default

    BATCH_SIZE = 500
    DEBOUNCE = 10.minutes

    def perform(workspace_id)
      workspace = Workspace.find_by(id: workspace_id)
      return unless workspace

      workspace.contacts
               .where("sender_kind_source IS DISTINCT FROM ?", "taught")
               .where.not(email_count: 0)
               .in_batches(of: BATCH_SIZE) do |batch|
        batch.each { |contact| Contacts::SenderKind.classify(contact) }
      end
      People::StandingsRefreshJob.enqueue_for_workspace(workspace)
    end

    # Enqueue at most once per DEBOUNCE window for a workspace (a set-if-absent
    # cache write is the gate — a no-op cache always enqueues, fine in dev/test).
    def self.enqueue_for(workspace_id, debounce: DEBOUNCE)
      return if workspace_id.blank?

      if debounce
        gate = Rails.cache.write("sender_kind_backfill_#{workspace_id}", true,
                                 expires_in: debounce, unless_exist: true)
        return unless gate
      end

      perform_later(workspace_id)
    end
  end
end
