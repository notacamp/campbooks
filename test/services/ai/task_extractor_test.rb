# frozen_string_literal: true

require "test_helper"

module Ai
  class TaskExtractorTest < ActiveSupport::TestCase
    test "re-raises transient provider errors so the job's retry_on gets its turn" do
      adapter = Object.new
      def adapter.chat(**) = raise Faraday::TooManyRequestsError, "429 from provider"

      with_config(adapter) do
        assert_raises(Faraday::TooManyRequestsError) do
          TaskExtractor.new(source: nil, content: "please send the file", workspace: nil).extract
        end
      end
    end

    test "degrades to [] on non-transient failure" do
      adapter = Object.new
      def adapter.chat(**) = raise "malformed everything"

      with_config(adapter) do
        assert_equal [], TaskExtractor.new(source: nil, content: "please send the file", workspace: nil).extract
      end
    end

    test "known tasks render as an exclusion block in the user message" do
      extractor = TaskExtractor.new(
        source: nil, content: "please send the file", workspace: nil,
        known_tasks: [ "Send the signed contract", "" ]
      )

      message = extractor.send(:user_message)

      assert_includes message, "<already_tracked_tasks>"
      assert_includes message, "- Send the signed contract"
    end

    test "no exclusion block when there are no known tasks" do
      extractor = TaskExtractor.new(source: nil, content: "please send the file", workspace: nil)

      refute_includes extractor.send(:user_message), "<already_tracked_tasks>"
    end

    private

    # Swap Ai::Configuration.for_any for the duration of the block (the suite has
    # no mocking gem; mirrors event_classification_job_test's singleton swap).
    def with_config(adapter)
      original = Ai::Configuration.method(:for_any)
      Ai::Configuration.define_singleton_method(:for_any) { |*| { adapter: adapter, model: "m" } }
      yield
    ensure
      Ai::Configuration.define_singleton_method(:for_any, original)
    end
  end
end
