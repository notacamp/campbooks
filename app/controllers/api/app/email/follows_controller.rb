# frozen_string_literal: true

module Api
  module App
    module Email
      # Subscribe / unsubscribe the acting user to a message's thread discussion.
      # Mirrors v1 EmailThreadsController#follow / #unfollow but routes through
      # the email message id (the SPA always has a message id at hand).
      class FollowsController < BaseController
        before_action :set_thread

        def create
          agent_thread = @thread.agent_thread || @thread.create_agent_thread!(
            title: @thread.subject,
            purpose: :email_chat,
            user: current_user,
            workspace: current_workspace
          )
          ::ThreadFollow.find_or_create_by!(user: current_user, agent_thread: agent_thread)
          render_data({ following: true, thread_id: @thread.id })
        end

        def destroy
          if (agent_thread = @thread.agent_thread)
            ::ThreadFollow.where(user: current_user, agent_thread: agent_thread).destroy_all
          end
          render_data({ following: false, thread_id: @thread.id })
        end

        private

        def set_thread
          message = ::EmailMessage.accessible_to(current_user).find(params[:email_message_id])
          @thread = message.email_thread
          raise ::ActiveRecord::RecordNotFound unless @thread
        end
      end
    end
  end
end
