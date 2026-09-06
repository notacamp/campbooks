# frozen_string_literal: true

require "rails_helper"

# A rate limit (or any provider hiccup) is the provider's problem, not the
# file's: it must reach the job's retry instead of becoming "AI parsing failed".
RSpec.describe Ai::BankStatementParser, "when the AI provider is unavailable" do
  let(:workspace) { Workspace.create!(name: "Parser WS") }
  let(:document) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("%PDF-1.4 fake"), filename: "statement.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  let(:adapter) { instance_double(Ai::Adapters::Openai) }
  let(:config)  { { adapter: adapter, provider: "openai", model: "gpt-4.1", max_tokens: 4000 } }
  let(:parser)  { described_class.new(document) }

  before do
    allow(Ai::Configuration).to receive(:for).with("document_analysis").and_return(config)
    allow(parser).to receive(:page_count).and_return(1)
    allow(parser).to receive(:rasterize_page).and_return({ type: :image, media_type: "image/jpeg", data: "x" })
    Current.workspace = workspace
  end

  after { Current.workspace = nil }

  it "lets a rate limit through untouched so the job can retry it" do
    allow(adapter).to receive(:chat).and_raise(Faraday::TooManyRequestsError.new("the server responded with status 429"))
    expect { parser.call }.to raise_error(Faraday::TooManyRequestsError)
  end

  it "lets a provider outage and a timeout through as well" do
    allow(adapter).to receive(:chat).and_raise(Faraday::ServerError.new("the server responded with status 503"))
    expect { parser.call }.to raise_error(Faraday::ServerError)

    allow(adapter).to receive(:chat).and_raise(Faraday::TimeoutError.new("timeout"))
    expect { parser.call }.to raise_error(Faraday::TimeoutError)
  end

  it "still turns an unexpected error into a parse error the user can read" do
    allow(adapter).to receive(:chat).and_raise(RuntimeError.new("boom"))
    expect { parser.call }.to raise_error(Reconciliations::ParseError, /AI parsing failed: boom/)
  end
end
