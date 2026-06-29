require "test_helper"

module Tasks
  class BuilderTest < ActiveSupport::TestCase
    setup do
      @ws = Workspace.create!(name: "Builder WS")
      # Any persisted record works as a polymorphic source for the builder's logic;
      # a user keeps the test free of email-account/document fixtures.
      @source = @ws.users.create!(name: "Src", email_address: "src-builder@example.com", password: "password123")
    end

    test "materializes a suggested, ai_suggested task from a raw item" do
      items = [ { "title" => "Sign the contract", "confidence" => 0.9, "priority" => "high", "due_date" => "2026-07-10" } ]

      tasks = Builder.call(workspace: @ws, source: @source, raw_items: items)

      assert_equal 1, tasks.size
      task = tasks.first
      assert task.suggested?
      assert task.ai_suggested?
      assert task.priority_high?
      assert_equal "Sign the contract", task.title
      assert_equal @source, task.source
    end

    test "is idempotent across re-extraction of the same source + title" do
      items = [ { "title" => "Review proposal", "confidence" => 0.8 } ]
      Builder.call(workspace: @ws, source: @source, raw_items: items)

      assert_no_difference -> { Task.count } do
        Builder.call(workspace: @ws, source: @source, raw_items: items)
      end
    end

    test "never overwrites a task the user already triaged" do
      items = [ { "title" => "Send invoice", "confidence" => 0.9 } ]
      task = Builder.call(workspace: @ws, source: @source, raw_items: items).first
      task.update!(status: :in_progress)

      # Same fingerprint (title is normalized for case/whitespace) — must not reset.
      Builder.call(workspace: @ws, source: @source, raw_items: [ { "title" => "  SEND INVOICE  ", "confidence" => 0.9 } ])

      assert task.reload.in_progress?
    end

    test "drops items below the confidence floor" do
      items = [ { "title" => "Maybe do this", "confidence" => 0.2 } ]
      assert_empty Builder.call(workspace: @ws, source: @source, raw_items: items)
    end

    test "keeps a past-due date — an overdue action still needs doing" do
      items = [ { "title" => "Overdue thing", "confidence" => 0.9, "due_date" => 3.days.ago.to_date.iso8601 } ]
      task = Builder.call(workspace: @ws, source: @source, raw_items: items).first

      assert_not_nil task.due_at
      assert task.due_at < Time.current
    end

    test "the email-linking actions are registered" do
      assert EmailActions.definition("create_task_from_email")
      assert EmailActions.definition("link_task_to_email")
    end
  end
end
