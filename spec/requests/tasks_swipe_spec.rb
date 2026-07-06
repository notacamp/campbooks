# frozen_string_literal: true

require "rails_helper"

# Verifies the turbo_stream swipe branches added to TasksController.
# A swipe request sends params[:swipe]=1 and expects turbo_stream.remove
# of dom_id(@task, :list_item) plus a notify toast.
RSpec.describe "Tasks swipe actions", type: :request do
  include ActionView::RecordIdentifier
  include ActiveJob::TestHelper

  before do
    @workspace = Workspace.create!(name: "Swipe Tasks WS")
    @user = @workspace.users.create!(
      name: "Tester",
      email_address: "swipe-tasks-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    @task = @workspace.tasks.create!(title: "Ship it", status: :todo, created_by: @user)
    sign_in(@user)
  end

  # ── Complete ─────────────────────────────────────────────────────────────

  it "complete swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(dom_id(@task, :list_item))
      expect(response.content_type).to match("turbo-stream")
    end
  end

  it "complete without swipe returns just a notify stream (no remove)" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include(dom_id(@task, :list_item))
    end
  end

  it "complete swipe marks the task as done" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(@task.reload.status).to eq("done")
    end
  end

  # ── Archive ──────────────────────────────────────────────────────────────

  it "archive swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(dom_id(@task, :list_item))
    end
  end

  it "archive without swipe responds with notify stream only" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include(dom_id(@task, :list_item))
    end
  end

  it "archive swipe marks the task as archived" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(@task.reload.archived?).to be_truthy
    end
  end

  # ── Destroy ──────────────────────────────────────────────────────────────

  it "destroy swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      delete task_path(@task, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(dom_id(@task, :list_item))
    end
  end

  it "destroy without swipe responds with notify stream only" do
    with_env("ENABLE_TASKS" => "1") do
      delete task_path(@task),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include(dom_id(@task, :list_item))
    end
  end

  it "destroy swipe deletes the task" do
    with_env("ENABLE_TASKS" => "1") do
      expect {
        delete task_path(@task, swipe: 1),
               headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change(Task, :count).by(-1)
    end
  end

  it "destroy requires authentication" do
    with_env("ENABLE_TASKS" => "1") do
      delete session_path
      delete task_path(@task, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:found)
    end
  end
end
