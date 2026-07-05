# frozen_string_literal: true

require "test_helper"

module Feed
  module Sources
    class DigestIssueTest < ActiveSupport::TestCase
      setup do
        @ws   = Workspace.create!(name: "Feed Digest WS")
        @user = @ws.users.create!(
          name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
          password: "password123"
        )
        @digest = @ws.scheduled_digests.create!(
          user:         @user,
          name:         "Feed digest",
          rrule:        "FREQ=WEEKLY",
          next_run_at:  1.week.from_now,
          config:       { "sources" => [ { "type" => "emails", "query" => "" } ] },
          show_in_feed: true
        )
        @source = Feed::Sources::DigestIssue.new(@user)
      end

      def generated_issue(attrs = {})
        @digest.issues.create!({
          workspace_id: @ws.id,
          user_id:      @user.id,
          period_start: 1.week.ago,
          period_end:   Time.current,
          status:       :generated,
          content:      { "overview" => "All good", "sections" => [], "meta" => {} }
        }.merge(attrs))
      end

      test "returns candidates for generated issues within the window" do
        issue = generated_issue
        candidates = with_digests_flag { @source.candidates }

        assert_not_empty candidates
        subjects = candidates.map { |c| c[:subject].id }
        assert_includes subjects, issue.id
      end

      test "excludes empty or failed issues" do
        generated_issue(status: :empty)
        generated_issue(status: :failed)

        candidates = with_digests_flag { @source.candidates }
        assert_empty candidates
      end

      test "excludes issues older than 3 days" do
        old_issue = generated_issue
        old_issue.update_column(:created_at, 4.days.ago)

        candidates = with_digests_flag { @source.candidates }
        subjects = candidates.map { |c| c[:subject].id }
        assert_not_includes subjects, old_issue.id
      end

      test "excludes issues when show_in_feed is false" do
        @digest.update!(show_in_feed: false)
        generated_issue

        candidates = with_digests_flag { @source.candidates }
        assert_empty candidates
      end

      test "excludes issues when digest is disabled" do
        @digest.update!(enabled: false)
        generated_issue

        candidates = with_digests_flag { @source.candidates }
        assert_empty candidates
      end

      test "returns empty array when feature flag is off" do
        with_env("ENABLE_DIGESTS" => nil) do
          generated_issue
          assert_empty @source.candidates
        end
      end

      test "candidate shape has expected keys" do
        issue = generated_issue
        candidates = with_digests_flag { @source.candidates }
        c = candidates.find { |x| x[:subject].id == issue.id }

        assert_not_nil c
        assert_equal "digest_issue:#{issue.id}", c[:dedupe_key]
        assert_equal 60, c[:score]
        assert_equal false, c[:attention]
        assert_equal @digest.name, c[:data]["digest_name"]
      end

      test "still_valid? returns true for generated issue" do
        issue = generated_issue
        assert @source.still_valid?(nil, issue)
      end

      test "still_valid? returns false for nil subject" do
        assert_not @source.still_valid?(nil, nil)
      end

      test "still_valid? returns false for non-generated issue" do
        issue = generated_issue
        issue.update!(status: :empty)
        assert_not @source.still_valid?(nil, issue)
      end

      private

      def with_digests_flag(&block)
        with_env("ENABLE_DIGESTS" => "1", &block)
      end
    end
  end
end
