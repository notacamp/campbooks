# frozen_string_literal: true

require "test_helper"

# Verifies the turbo_stream swipe branches added to TasksController.
# A swipe request sends params[:swipe]=1 and expects turbo_stream.remove
# of dom_id(@task, :list_item) plus a notify toast.
class TasksSwipeTest < ActionDispatch::IntegrationTest
  include ActionView::RecordIdentifier
  include ActiveJob::TestHelper

  setup do
    @workspace = Workspace.create!(name: "Swipe Tasks WS")
    @user = @workspace.users.create!(
      name: "Tester",
      email_address: "swipe-tasks-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    @task = @workspace.tasks.create!(title: "Ship it", status: :todo, created_by: @user)
    sign_in(@user)
  end

  # ── Complete ─────────────────��────────────────────────────────────────────

  test "complete swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_includes response.body, dom_id(@task, :list_item)
      assert_match "turbo-stream", response.content_type
    end
  end

  test "complete without swipe returns just a notify stream (no remove)" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_not_includes response.body, dom_id(@task, :list_item)
    end
  end

  test "complete swipe marks the task as done" do
    with_env("ENABLE_TASKS" => "1") do
      patch complete_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_equal "done", @task.reload.status
    end
  end

  # ── Archive ─────────────���────────────────────────────────────────────────

  test "archive swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_includes response.body, dom_id(@task, :list_item)
    end
  end

  test "archive without swipe responds with notify stream only" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_not_includes response.body, dom_id(@task, :list_item)
    end
  end

  test "archive swipe marks the task as archived" do
    with_env("ENABLE_TASKS" => "1") do
      patch archive_task_path(@task, swipe: 1),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert @task.reload.archived?
    end
  end

  # ── Destroy ────────────────────��───────────────────────────��──────────────

  test "destroy swipe removes the list_item row and shows a toast" do
    with_env("ENABLE_TASKS" => "1") do
      delete task_path(@task, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_includes response.body, dom_id(@task, :list_item)
    end
  end

  test "destroy without swipe responds with notify stream only" do
    with_env("ENABLE_TASKS" => "1") do
      delete task_path(@task),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :success
      assert_not_includes response.body, dom_id(@task, :list_item)
    end
  end

  test "destroy swipe deletes the task" do
    with_env("ENABLE_TASKS" => "1") do
      assert_difference -> { Task.count }, -1 do
        delete task_path(@task, swipe: 1),
               headers: { "Accept" => "text/vnd.turbo-stream.html" }
      end
    end
  end

  test "destroy requires authentication" do
    with_env("ENABLE_TASKS" => "1") do
      delete session_path
      delete task_path(@task, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

      assert_response :redirect
    end
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end
end
