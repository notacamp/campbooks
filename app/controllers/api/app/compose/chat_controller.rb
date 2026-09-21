# frozen_string_literal: true

module Api
  module App
    module Compose
      # Scout AI compose-chat. The SPA POSTs a turn; this endpoint creates the
      # user AgentMessage and enqueues ComposeChatReplyJob to generate the AI
      # reply asynchronously. The response is 201 with the new message id +
      # thread id; the SPA polls the Scout messages endpoint for the AI reply.
      #
      # Streaming draft tokens (SSE / ActionCable) is a future enhancement
      # tracked in api-migration/compose-email.md § Open questions.
      class ChatController < BaseController
        def create
          thread = find_or_create_thread
          message = thread.agent_messages.create!(
            content: params.require(:content),
            author_type: :user,
            user: current_user
          )
          ::ComposeChatReplyJob.perform_later(message.id)
          render_data(
            { id: message.id, thread_id: thread.id, content: message.content, status: "processing" },
            status: :created
          )
        end

        private

        def find_or_create_thread
          if params[:thread_id].present?
            current_user.agent_threads.find_by(id: params[:thread_id]) || create_thread
          else
            create_thread
          end
        end

        def create_thread
          current_user.agent_threads.create!(
            title: "Compose: #{::Time.current.strftime('%b %d, %H:%M')}",
            workspace: current_workspace
          )
        end
      end
    end
  end
end
