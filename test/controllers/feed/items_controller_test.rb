# frozen_string_literal: true

require "test_helper"

module Feed
  class ItemsControllerTest < ActionDispatch::IntegrationTest
    setup do
      @workspace = Workspace.create!(name: "Feed Items WS")
      @user = @workspace.users.create!(
        name: "Reader", email_address: "feed-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      post session_path, params: { email_address: @user.email_address, password: "password123" }

      @task = Task.create!(
        workspace: @workspace, title: "Send the signed statements",
        status: :suggested, priority: :normal, ai_suggested: true, confidence: 0.9
      )
      @item = FeedItem.create!(
        user: @user, workspace: @workspace, kind: "task", subject: @task,
        dedupe_key: "task_suggestion:#{@task.id}", sort_at: Time.current
      )
    end

    test "accept promotes the suggestion to todo and resolves the card" do
      post act_feed_item_path(@item, format: :turbo_stream), params: { tool: "accept" }

      assert_response :success
      assert @task.reload.todo?
      assert @item.reload.acted?
    end

    test "dismiss_task cancels the suggestion" do
      post act_feed_item_path(@item, format: :turbo_stream), params: { tool: "dismiss_task" }

      assert_response :success
      assert @task.reload.cancelled?
      assert @item.reload.acted?
    end

    test "another user's item 404s" do
      other = @workspace.users.create!(
        name: "Other", email_address: "other-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      foreign = FeedItem.create!(
        user: other, workspace: @workspace, kind: "task", subject: @task,
        dedupe_key: "task_suggestion:other", sort_at: Time.current
      )

      post act_feed_item_path(foreign, format: :turbo_stream), params: { tool: "accept" }

      assert_response :not_found
      assert @task.reload.suggested?
    end
  end
end
