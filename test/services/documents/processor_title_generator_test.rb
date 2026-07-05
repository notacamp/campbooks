# frozen_string_literal: true

require "test_helper"

module Documents
  # Tests that Documents::Processor invokes TitleGenerator after a successful AI
  # analysis pass, and that no email-address titled docs slip through.
  class ProcessorTitleGeneratorTest < ActiveSupport::TestCase
    setup do
      @ws = Workspace.create!(name: "Processor TG WS")
    end

    def build_doc(sender_name: nil, metadata: nil)
      doc = @ws.documents.new(
        document_type: "other",
        ai_status: :pending,
        review_status: :pending,
        source: :email,
        sender_name: sender_name,
        metadata: metadata
      )
      doc.original_file.attach(io: StringIO.new("data"), filename: "test.pdf", content_type: "application/pdf")
      doc.save!
      doc
    end

    def stub_analyzer_success(doc)
      original = Ai::DocumentAnalyzer.instance_method(:call)
      Ai::DocumentAnalyzer.define_method(:call) do
        doc.update_columns(ai_status: Document.ai_statuses[:completed],
                           metadata: (doc.metadata || {}).merge("title" => "Vendor Invoice — Jan 2026"))
        { error: nil }
      end
      yield
    ensure
      Ai::DocumentAnalyzer.define_method(:call, original)
    end

    def stub_analyzer_failure(doc)
      original = Ai::DocumentAnalyzer.instance_method(:call)
      Ai::DocumentAnalyzer.define_method(:call) do
        doc.update_columns(ai_status: Document.ai_statuses[:failed], ai_error: "timeout")
        { error: "timeout" }
      end
      yield
    ensure
      Ai::DocumentAnalyzer.define_method(:call, original)
    end

    def stub_title_generator_called
      calls = []
      original = Documents::TitleGenerator.instance_method(:call)
      Documents::TitleGenerator.define_method(:call) do
        calls << @document.id
        nil
      end
      yield calls
    ensure
      Documents::TitleGenerator.define_method(:call, original)
    end

    test "Processor calls TitleGenerator after a successful AI pass" do
      doc = build_doc
      stub_analyzer_success(doc) do
        stub_title_generator_called do |calls|
          Documents::Processor.new(doc).call
          assert_includes calls, doc.id
        end
      end
    end

    test "Processor does NOT call TitleGenerator when AI fails (returns early)" do
      doc = build_doc
      stub_analyzer_failure(doc) do
        stub_title_generator_called do |calls|
          Documents::Processor.new(doc).call
          assert_empty calls
        end
      end
    end
  end
end
