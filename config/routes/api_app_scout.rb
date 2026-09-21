# frozen_string_literal: true

# Scout overlay (idle suggestions), chat threads/messages, tool invocation. See api-migration/scout.md.
# Owned by the api_app_scout build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/scout/.
namespace :api do
  namespace :app do
    namespace :scout do
      # GET  /api/app/scout/overlay
      # Idle-state payload: AI-available flag, briefing suggestions, last 6 threads.
      resource :overlay, only: :show, controller: "overlay"

      # GET  /api/app/scout/unread
      # Count of unviewed AI replies (drives the docked-bar dot).
      resource :unread, only: :show, controller: "unread"

      # POST /api/app/scout/mark_read
      # Mark all AI replies as viewed (clears the docked-bar dot).
      resource :mark_read, only: :create, controller: "mark_read"

      # POST /api/app/scout/tool
      # Execute a confirm-level tool on behalf of a user click.
      resource :tool, only: :create, controller: "tool"

      # Thread CRUD + message async chat.
      resources :threads, controller: "threads" do
        resources :messages, only: %i[index create], controller: "messages"
      end
    end
  end
end
