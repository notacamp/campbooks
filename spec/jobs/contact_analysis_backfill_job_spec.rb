# frozen_string_literal: true

require "rails_helper"

RSpec.describe ContactAnalysisBackfillJob, type: :job do
  # Flip the AI-provider guard without minitest/mock. Saves and restores the real
  # class method.
  def with_provider_configured(value)
    sc = Ai::ProviderSetup.singleton_class
    original = sc.instance_method(:configured?)
    sc.send(:define_method, :configured?) { |*| value }
    yield
  ensure
    sc.send(:define_method, :configured?, original)
  end

  it "enqueues analysis for un-analyzed, past-threshold contacts across workspaces" do
    ws = Workspace.create!(name: "Backfill Job WS")
    ws.contacts.create!(email: "a@example.com", email_count: 9)
    ws.contacts.create!(email: "b@example.com", email_count: 9, analyzed_at: Time.current) # already analyzed — skip
    ws.contacts.create!(email: "c@example.com", email_count: 1)                            # below threshold — skip

    with_provider_configured(true) do
      expect { described_class.perform_now }
        .to have_enqueued_job(ContactAnalysisJob).exactly(1).times
    end
  end
end
