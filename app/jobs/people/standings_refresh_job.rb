# frozen_string_literal: true

module People
  # Refreshes the materialized People::Standings for one user. Triggered by
  # Feed::RefreshJob (after every feed refresh), Contacts::SenderKindBackfillJob
  # (a sender-kind verdict changes who is eligible), and the People request path
  # when standings are stale.
  #
  # Mirrors Feed::RefreshJob: .enqueue_for debounces bursts via a short cache
  # window so that rapid triggers fold into one run.
  class StandingsRefreshJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    discard_on ActiveRecord::RecordNotFound

    DEBOUNCE = 30.seconds

    def perform(user_id)
      user = User.find_by(id: user_id)
      return unless user&.workspace_id

      People::Standings.refresh!(user)
    end

    # Enqueue a refresh for one user, at most once per DEBOUNCE window. The
    # set-if-absent cache write is the debounce gate (a no-op cache always
    # enqueues, which is fine in dev/test).
    def self.enqueue_for(user_id, debounce: DEBOUNCE)
      return if user_id.blank?

      if debounce
        gate = Rails.cache.write("people_standings_pending_#{user_id}", true,
                                 expires_in: debounce, unless_exist: true)
        return unless gate
      end

      perform_later(user_id)
    end

    # Enqueue a refresh for every user in the workspace (a sender-kind verdict
    # changes who appears in the directory).
    def self.enqueue_for_workspace(workspace)
      return unless workspace

      workspace.users.pluck(:id).each { |uid| enqueue_for(uid) }
    end
  end
end
