# frozen_string_literal: true

module Api
  module V1
    # Public API for email threads (conversations). The list is the acting user's
    # readable mailboxes' threads, newest activity first; show returns the whole
    # conversation with its messages in order. Follow/unfollow subscribe the
    # acting user to a thread's discussion (the same ThreadFollow the web
    # @mention flow uses), so they get notified even without mailbox access.
    class EmailThreadsController < BaseController
      before_action -> { doorkeeper_authorize! :"emails:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"emails:write" }, only: [ :follow, :unfollow ]
      before_action :set_thread, only: [ :show, :follow, :unfollow ]

      def index
        readable_ids = Current.user.readable_email_accounts.select(:id)
        scope = EmailThread.where(email_account_id: readable_ids)
                           .includes(:tags, :agent_thread, email_messages: :tags)
                           .order(updated_at: :desc)

        @pagy, threads = pagy(scope, limit: per_page)
        render_page(
          threads.map { |t| ThreadSerializer.new(t, user: Current.user).as_json },
          @pagy
        )
      end

      def show
        render_data(ThreadSerializer.new(@thread, detail: true, user: Current.user).as_json)
      end

      # Subscribe the acting user to this thread's discussion. Lazily creates the
      # backing AgentThread (an email has none until the first comment or follow),
      # mirroring DiscussionThreadable#find_or_create_agent_thread.
      def follow
        agent_thread = @thread.agent_thread || @thread.create_agent_thread!(
          title: @thread.subject,
          purpose: :email_chat,
          user: Current.user,
          workspace: Current.workspace
        )
        ThreadFollow.find_or_create_by!(user: Current.user, agent_thread: agent_thread)
        render_data(ThreadSerializer.new(@thread.reload, detail: true, user: Current.user).as_json)
      end

      def unfollow
        if (agent_thread = @thread.agent_thread)
          ThreadFollow.where(user: Current.user, agent_thread: agent_thread).destroy_all
        end
        render_data(ThreadSerializer.new(@thread.reload, detail: true, user: Current.user).as_json)
      end

      private

      # Thread access = mailbox read access OR following the discussion. The
      # global find + explicit accessible_by? check 404s an out-of-reach thread
      # without revealing it exists (mirrors EmailThreadsController#show).
      def set_thread
        thread = EmailThread.find(params[:id])
        raise ActiveRecord::RecordNotFound unless thread.accessible_by?(Current.user)

        @thread = thread
      end
    end
  end
end
