# frozen_string_literal: true

require "test_helper"

module Contacts
  class PendingAnalysisCatchUpTest < ActiveSupport::TestCase
    include ActiveJob::TestHelper

    setup { @ws = Workspace.create!(name: "Contact CatchUp WS") }

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

    def build_contact(email_count:, analyzed_at: nil)
      @ws.contacts.create!(
        email: "c#{SecureRandom.hex(6)}@example.com",
        email_count: email_count,
        analyzed_at: analyzed_at
      )
    end

    test "enqueues analysis only for unanalyzed contacts past the threshold when a provider is configured" do
      with_provider_configured(true) do
        build_contact(email_count: Contacts::Identifier::FIRST_ANALYSIS_THRESHOLD)
        build_contact(email_count: 40)
        build_contact(email_count: 40, analyzed_at: Time.current) # already analyzed — skip
        build_contact(email_count: 2)                             # below threshold — skip

        assert_enqueued_jobs(2, only: ContactAnalysisJob) do
          Contacts::PendingAnalysisCatchUp.run(@ws)
        end
      end
    end

    test "enqueues nothing when no text provider is configured" do
      with_provider_configured(false) do
        build_contact(email_count: 10)
        assert_no_enqueued_jobs(only: ContactAnalysisJob) do
          Contacts::PendingAnalysisCatchUp.run(@ws)
        end
      end
    end

    test "caps each pass at LIMIT so a large backlog can't flood the queue" do
      with_provider_configured(true) do
        (PendingAnalysisCatchUp::LIMIT + 3).times { build_contact(email_count: 5) }

        assert_enqueued_jobs(PendingAnalysisCatchUp::LIMIT, only: ContactAnalysisJob) do
          Contacts::PendingAnalysisCatchUp.run(@ws)
        end
      end
    end

    test "is a safe no-op for a nil workspace" do
      assert_no_enqueued_jobs { Contacts::PendingAnalysisCatchUp.run(nil) }
    end
  end
end
