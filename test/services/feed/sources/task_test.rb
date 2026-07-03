# frozen_string_literal: true

require "test_helper"

module Feed
  module Sources
    class TaskTest < ActiveSupport::TestCase
      setup do
        @workspace = Workspace.create!(name: "Feed Task WS")
        @user = @workspace.users.create!(
          name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
          password: "password123"
        )
        @source = Feed::Sources::Task.new(@user)
      end

      test "confident suggestions surface with their own dedupe key" do
        task = suggested_task(confidence: 0.9)

        candidate = @source.candidates.find { |c| c[:subject] == task }

        assert candidate, "expected a candidate for the suggestion"
        assert_equal "task_suggestion:#{task.id}", candidate[:dedupe_key]
        assert_equal "suggested", candidate[:data]["status"]
      end

      test "low-confidence and stale suggestions stay off the feed" do
        low = suggested_task(confidence: 0.4)
        stale = suggested_task(confidence: 0.9)
        stale.update_column(:created_at, 20.days.ago)

        subjects = @source.candidates.map { |c| c[:subject] }

        refute_includes subjects, low
        refute_includes subjects, stale
      end

      test "active tasks keep the plain task dedupe key" do
        task = ::Task.create!(workspace: @workspace, title: "Ship it", status: :todo,
                              priority: :normal, due_at: 1.day.from_now)

        candidate = @source.candidates.find { |c| c[:subject] == task }

        assert candidate
        assert_equal "task:#{task.id}", candidate[:dedupe_key]
      end

      test "still_valid? tracks the flavor: suggestions while suggested, actives while active" do
        task = suggested_task(confidence: 0.9)
        suggestion_item = FeedItem.new(dedupe_key: "task_suggestion:#{task.id}")
        active_item = FeedItem.new(dedupe_key: "task:#{task.id}")

        assert @source.still_valid?(suggestion_item, task)
        refute @source.still_valid?(active_item, task)

        task.move_to_status!(:todo, by: nil)
        refute @source.still_valid?(suggestion_item, task)
        assert @source.still_valid?(active_item, task)

        task.archive!(by: nil)
        refute @source.still_valid?(active_item, task)
        refute @source.still_valid?(suggestion_item, nil)
      end

      private

      def suggested_task(confidence:)
        ::Task.create!(
          workspace: @workspace, title: "Suggested #{SecureRandom.hex(3)}",
          status: :suggested, priority: :normal, ai_suggested: true, confidence: confidence
        )
      end
    end
  end
end
