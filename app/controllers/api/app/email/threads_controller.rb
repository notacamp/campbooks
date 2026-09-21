# frozen_string_literal: true

module Api
  module App
    module Email
      # Email thread reads for /api/app. Index returns the acting user's thread
      # list; show returns the full thread with message chain (marks read) and
      # the user's follow state.
      class ThreadsController < BaseController
        before_action :set_thread, only: %i[show follow unfollow]

        def index
          readable_ids = current_user.readable_email_accounts.select(:id)
          scope = ::EmailThread
                    .where(email_account_id: readable_ids)
                    .includes(:tags, :agent_thread, email_messages: :tags)
                    .order(updated_at: :desc)
          @pagy, threads = pagy(scope, limit: per_page)
          render_page(
            threads.map { |t| ThreadSerializer.new(t, user: current_user).as_json },
            @pagy
          )
        end

        def show
          ::Emails::MarkThreadRead.call(@thread)
          render_data(
            ThreadSerializer.new(@thread.reload, detail: true, user: current_user).as_json
          )
        end

        def follow
          agent_thread = @thread.agent_thread || @thread.create_agent_thread!(
            title: @thread.subject,
            purpose: :email_chat,
            user: current_user,
            workspace: current_workspace
          )
          ::ThreadFollow.find_or_create_by!(user: current_user, agent_thread: agent_thread)
          render_data(ThreadSerializer.new(@thread.reload, detail: true, user: current_user).as_json)
        end

        def unfollow
          if (agent_thread = @thread.agent_thread)
            ::ThreadFollow.where(user: current_user, agent_thread: agent_thread).destroy_all
          end
          render_data(ThreadSerializer.new(@thread.reload, detail: true, user: current_user).as_json)
        end

        private

        def set_thread
          thread = ::EmailThread.find(params[:id])
          raise ::ActiveRecord::RecordNotFound unless thread.accessible_by?(current_user)

          @thread = thread
        end
      end
    end
  end
end
