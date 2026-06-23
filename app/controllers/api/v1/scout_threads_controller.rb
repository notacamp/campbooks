# frozen_string_literal: true

module Api
  module V1
    # Public API for Scout chat threads. Threads are per-user (the credential acts
    # as its creating user), so everything is scoped through current_user — a
    # client can only see and create threads for its own acting user. Setup chats
    # (an internal onboarding aid) are hidden via the scout_visible scope.
    class ScoutThreadsController < BaseController
      before_action -> { doorkeeper_authorize! :"scout:read" },  only: :index
      before_action -> { doorkeeper_authorize! :"scout:write" }, only: :create

      def index
        scope = current_user.agent_threads.scout_visible.recent
        @pagy, threads = pagy(scope, limit: per_page)
        render_page(threads.map { |thread| AgentThreadSerializer.new(thread).as_json }, @pagy)
      end

      def create
        thread = current_user.agent_threads.create!(
          title: params[:title].presence || "New chat",
          workspace_id: current_user.workspace_id
          # purpose defaults to :global
        )
        render_data(AgentThreadSerializer.new(thread).as_json, status: :created)
      end
    end
  end
end
