# frozen_string_literal: true

require "test_helper"

module Documents
  class PendingAnalysisCatchUpTest < ActiveSupport::TestCase
    include ActiveJob::TestHelper

    setup { @ws = Workspace.create!(name: "CatchUp WS") }

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

    def build_doc(ai_status)
      doc = @ws.documents.new(document_type: "other", ai_status: ai_status,
                              review_status: :pending, source: :manual_upload)
      doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
      doc.save!
      doc
    end

    test "enqueues analysis only for ai_pending docs when a provider is configured" do
      with_provider_configured(true) do
        build_doc(:pending)
        build_doc(:pending)
        build_doc(:completed) # already analyzed — must be skipped

        assert_enqueued_jobs(2, only: DocumentProcessJob) do
          Documents::PendingAnalysisCatchUp.run(@ws)
        end
      end
    end

    test "enqueues nothing when no document provider is configured" do
      with_provider_configured(false) do
        build_doc(:pending)
        assert_no_enqueued_jobs { Documents::PendingAnalysisCatchUp.run(@ws) }
      end
    end

    test "caps each pass at LIMIT so a large backlog can't flood the queue" do
      with_provider_configured(true) do
        (PendingAnalysisCatchUp::LIMIT + 3).times { build_doc(:pending) }

        assert_enqueued_jobs(PendingAnalysisCatchUp::LIMIT, only: DocumentProcessJob) do
          Documents::PendingAnalysisCatchUp.run(@ws)
        end
      end
    end

    test "is a safe no-op for a nil workspace" do
      assert_no_enqueued_jobs { Documents::PendingAnalysisCatchUp.run(nil) }
    end
  end
end
