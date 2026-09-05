# frozen_string_literal: true

module Attention
  # Recomputes attention weights for one user. Triggered by Feed::RefreshJob
  # (when weights are stale) and can be called directly for a single user or
  # backfilled across a workspace.
  #
  # Mirrors People::StandingsRefreshJob: .enqueue_for debounces bursts via a
  # short cache window so rapid triggers fold into one run.
  class RefreshJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    discard_on ActiveRecord::RecordNotFound

    DEBOUNCE = 30.seconds

    def perform(user_id)
      user = User.find_by(id: user_id)
      return unless user&.workspace_id

      Attention::Refresh.call(user)
    end

    # Enqueue a refresh for one user, at most once per DEBOUNCE window.
    def self.enqueue_for(user_id, debounce: DEBOUNCE)
      return if user_id.blank?

      if debounce
        gate = Rails.cache.write("attention_refresh_pending_#{user_id}", true,
                                 expires_in: debounce, unless_exist: true)
        return unless gate
      end

      perform_later(user_id)
    end

    # Enqueue for every user in the workspace.
    def self.enqueue_for_workspace(workspace)
      return unless workspace

      workspace.users.pluck(:id).each { |uid| enqueue_for(uid) }
    end
  end
end
