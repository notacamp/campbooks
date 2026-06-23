module Feed
  # Regenerates one user's feed (Feed::Generator). Triggered by event hooks
  # (email analyzed, document put in review) and a periodic sweep.
  #
  # Generation is idempotent, so it's safe to run often; .enqueue_for debounces
  # bursts (a scan analysing 50 emails shouldn't queue 50 regenerations) via a
  # short cache window, and any triggers inside that window fold into the one run.
  class RefreshJob < ApplicationJob
    queue_as :default
    discard_on ActiveJob::DeserializationError

    DEBOUNCE = 30.seconds

    def perform(user_id)
      user = User.find_by(id: user_id)
      return unless user&.workspace_id

      Feed::Generator.for_user(user)
    end

    # Enqueue a refresh for one user, at most once per DEBOUNCE window. The
    # set-if-absent cache write is the debounce gate (no-op cache ⇒ always
    # enqueues, which is fine in dev/test).
    def self.enqueue_for(user_id, debounce: DEBOUNCE)
      return if user_id.blank?

      if debounce
        gate = Rails.cache.write("feed_refresh_pending_#{user_id}", true,
                                 expires_in: debounce, unless_exist: true)
        return unless gate
      end

      perform_later(user_id)
    end

    # Refresh everyone who can read this mailbox (a new email was analysed there).
    def self.enqueue_for_account(account)
      return unless account

      account.email_account_users.where(can_read: true).distinct.pluck(:user_id).each do |uid|
        enqueue_for(uid)
      end
    end

    # Refresh every member of the workspace (a workspace-scoped document changed).
    def self.enqueue_for_workspace(workspace)
      return unless workspace

      workspace.users.pluck(:id).each { |uid| enqueue_for(uid) }
    end
  end
end
