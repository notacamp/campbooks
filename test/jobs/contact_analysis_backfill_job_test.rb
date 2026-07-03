# frozen_string_literal: true

require "test_helper"

class ContactAnalysisBackfillJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  # Flip the AI-provider guard without minitest/mock (which trips this project's
  # test runner). Saves and restores the real class method.
  def with_provider_configured(value)
    sc = Ai::ProviderSetup.singleton_class
    original = sc.instance_method(:configured?)
    sc.send(:define_method, :configured?) { |*| value }
    yield
  ensure
    sc.send(:define_method, :configured?, original)
  end

  test "enqueues analysis for un-analyzed, past-threshold contacts across workspaces" do
    ws = Workspace.create!(name: "Backfill Job WS")
    ws.contacts.create!(email: "a@example.com", email_count: 9)
    ws.contacts.create!(email: "b@example.com", email_count: 9, analyzed_at: Time.current) # already analyzed — skip
    ws.contacts.create!(email: "c@example.com", email_count: 1)                            # below threshold — skip

    with_provider_configured(true) do
      assert_enqueued_jobs(1, only: ContactAnalysisJob) do
        ContactAnalysisBackfillJob.perform_now
      end
    end
  end
end
