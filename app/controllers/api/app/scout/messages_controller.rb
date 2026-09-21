# frozen_string_literal: true

module Api
  module App
    module Scout
      # Async message posting inside a Scout thread. Mirrors the /api/v1 pattern:
      # POST user message → 202 → AI reply enqueued in AgentChatReplyJob →
      # poll GET …/messages?after_message_id=<id> to receive the AI reply.
      #
      # No ActionCable/Turbo broadcasts from the API layer — the SPA polls.
      class MessagesController < Api::App::BaseController
        before_action :set_thread

        # GET /api/app/scout/threads/:thread_id/messages
        # Returns all messages in the thread, optionally filtered by ?after_message_id=.
        # Pass ?after_message_id=<id> to poll for the async AI reply.
        def index
          scope = @thread.agent_messages.chronological

          if params[:after_message_id].present?
            pivot = @thread.agent_messages.find_by(id: params[:after_message_id])
            scope = scope.where("agent_messages.created_at > ?", pivot.created_at) if pivot
          end

          render_data(scope.map { |m| Api::App::Scout::AgentMessageSerializer.new(m).as_json })
        end

        # POST /api/app/scout/threads/:thread_id/messages
        # Creates the user message and enqueues the AI reply. Returns 202 with the
        # created message so the client can optimistically render it immediately.
        def create
          unless Ai::ProviderSetup.available?(current_workspace, :text)
            return render_error("ai_provider_unconfigured",
                                "This workspace has no AI provider configured for chat.",
                                status: :service_unavailable)
          end

          message = @thread.agent_messages.create!(
            content: params.require(:content),
            author_type: :user,
            user: current_user
          )
          AgentChatReplyJob.perform_later(message.id)

          render_data(Api::App::Scout::AgentMessageSerializer.new(message).as_json, status: :accepted)
        end

        private

        def set_thread
          @thread = current_user.agent_threads.scout_visible.find(params[:thread_id])
        end
      end
    end
  end
end
