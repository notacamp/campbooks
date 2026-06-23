# frozen_string_literal: true

module Api
  module V1
    # Public API for messages within a Scout thread. The flow is asynchronous:
    # POST a user message → 202 + the created message → the AI reply is generated
    # in a background job (AgentChatReplyJob) → poll GET …/messages?after_message_id=<id>
    # for the reply (a new message with author_type "ai" and reply_status "replied").
    class ScoutMessagesController < BaseController
      before_action -> { doorkeeper_authorize! :"scout:read" },  only: :index
      before_action -> { doorkeeper_authorize! :"scout:write" }, only: :create
      before_action :set_thread

      # Chronological messages in the thread. Pass ?after_message_id=N to fetch only
      # messages created after that one — the poll loop for the async AI reply.
      def index
        scope = @thread.agent_messages.chronological

        if params[:after_message_id].present?
          pivot = @thread.agent_messages.find_by(id: params[:after_message_id])
          # Compare by created_at (not id) so it's robust across DB replicas.
          scope = scope.where("agent_messages.created_at > ?", pivot.created_at) if pivot
        end

        render json: { data: scope.map { |message| AgentMessageSerializer.new(message).as_json } }
      end

      # Post a user message and kick off the async AI reply. Fails closed (503) if
      # the workspace has no AI provider configured for chat, mirroring the web
      # AiProviderGuard so the request never reaches a doomed background job.
      def create
        unless Ai::ProviderSetup.available?(Current.workspace, :text)
          return render_api_error("ai_provider_unconfigured",
                                  "This workspace has no AI provider configured for chat.",
                                  status: :service_unavailable)
        end

        message = @thread.agent_messages.create!(
          content: params[:content],
          author_type: :user,
          user: current_user
        )
        AgentChatReplyJob.perform_later(message.id)

        render json: { data: AgentMessageSerializer.new(message).as_json }, status: :accepted
      end

      private

      def set_thread
        @thread = current_user.agent_threads.find(params[:thread_id])
      end
    end
  end
end
