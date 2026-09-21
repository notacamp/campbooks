# frozen_string_literal: true

module Api
  module App
    module Email
      # Screen-shaped email thread for the React SPA. Extends the v1 ThreadSerializer
      # with following state and a richer per-message stub in the detail view.
      class ThreadSerializer
        def initialize(thread, detail: false, user: nil)
          @thread = thread
          @detail = detail
          @user = user
        end

        def as_json(*)
          latest = @thread.latest_message
          data = {
            id: @thread.id,
            subject: @thread.display_subject,
            account_id: @thread.email_account_id,
            message_count: @thread.email_messages.size,
            unread: @thread.unread?,
            pinned: @thread.pinned?,
            snoozed: @thread.snoozed?,
            snoozed_until: @thread.snoozed_until&.iso8601,
            last_message_at: latest&.received_at&.iso8601,
            participants: @thread.participant_senders,
            tags: @thread.tags.map(&:name),
            holds_last_word: @thread.holds_last_word?,
            created_at: @thread.created_at&.iso8601,
            updated_at: @thread.updated_at&.iso8601
          }

          if @detail
            data[:following] = following?
            data[:messages] = @thread.email_messages
                                     .sort_by { |m| m.received_at || ::Time.at(0) }
                                     .map { |m| MessageSerializer.new(m, detail: true).as_json }
          end

          data
        end

        private

        def following?
          agent_thread = @thread.agent_thread
          return false unless @user && agent_thread

          ThreadFollow.exists?(user: @user, agent_thread: agent_thread)
        end
      end
    end
  end
end
