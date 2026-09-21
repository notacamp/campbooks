# frozen_string_literal: true

module Api
  module App
    module Scout
      # CRUD for Scout chat threads. Threads are per-user and scoped to
      # current_user. Setup chats (onboarding aids) are hidden via scout_visible.
      #
      # Thread list returns paginated threads with their last message embedded so
      # the SPA sidebar can show a preview without a second request.
      class ThreadsController < Api::App::BaseController
        before_action :set_thread, only: %i[show update destroy]

        # GET /api/app/scout/threads
        def index
          scope = current_user.agent_threads.scout_visible.with_messages.recent
          @pagy, threads = pagy(scope, limit: per_page)
          serialized = threads.map { |t| Api::App::Scout::AgentThreadSerializer.new(t).as_json }
          render_page(serialized, @pagy)
        end

        # POST /api/app/scout/threads
        def create
          thread = current_user.agent_threads.create!(
            title: params[:title].presence || "New chat",
            workspace_id: current_user.workspace_id
            # purpose defaults to :global
          )
          render_data(Api::App::Scout::AgentThreadSerializer.new(thread).as_json, status: :created)
        end

        # GET /api/app/scout/threads/:id
        # Returns the thread header + last 50 messages (screen-shaped for the
        # full Scout surface conversation panel).
        def show
          messages = @thread.agent_messages.chronological.last(50)
          render_data(
            Api::App::Scout::AgentThreadSerializer.new(@thread).as_json.merge(
              messages: messages.map { |m| Api::App::Scout::AgentMessageSerializer.new(m).as_json }
            )
          )
        end

        # PATCH /api/app/scout/threads/:id
        def update
          @thread.update!(title: params[:title]) if params[:title].present?
          render_data(Api::App::Scout::AgentThreadSerializer.new(@thread).as_json)
        end

        # DELETE /api/app/scout/threads/:id
        def destroy
          @thread.destroy!
          render json: {}, status: :no_content
        end

        private

        def set_thread
          @thread = current_user.agent_threads.scout_visible.find(params[:id])
        end
      end
    end
  end
end
