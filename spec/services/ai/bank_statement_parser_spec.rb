# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::BankStatementParser do
  let(:workspace) { Workspace.create!(name: "Parser WS") }
  let(:document) do
    doc = workspace.documents.build(
      document_type: :bank_statement,
      ai_status:     :skipped,
      review_status: :pending,
      source:        :manual_upload
    )
    doc.original_file.attach(
      io:           StringIO.new("%PDF-1.4 fake"),
      filename:     "statement.pdf",
      content_type: "application/pdf"
    )
    doc.save!
    doc
  end

  let(:adapter) { instance_double(Ai::Adapters::Openai) }
  let(:config)  { { adapter: adapter, provider: "openai", model: "gpt-4.1", max_tokens: 4000 } }

  before do
    allow(Ai::Configuration).to receive(:for).with("document_analysis").and_return(config)
    Current.workspace = workspace
  end

  after { Current.workspace = nil }

  describe "page rasterization (regressions: prod 2026-07-09 and 2026-07-10)" do
    # 07-09: bracket syntax passed to Image.open raised ENOENT on every page.
    # 07-10: the "fix" used Image#format + Image#page — canvas geometry, not a
    # page selector — producing junk thumbnails on multipage PDFs. The correct
    # mechanism is the convert tool with a bracketed INPUT and density BEFORE
    # the input. Real-ImageMagick coverage lives in pdf_rasterization_spec.rb;
    # this example pins the tool invocation shape without needing ImageMagick.
    it "renders via the convert tool: density first, bracketed input, explicit output" do
      recorded = []
      tool = double("convert")
      allow(tool).to receive(:density) { |v| recorded << [ :density, v ] }
      allow(tool).to receive(:quality) { |v| recorded << [ :quality, v ] }
      allow(tool).to receive(:<<)      { |v| recorded << [ :arg, v ] }
      allow(MiniMagick).to receive(:convert) { |&blk| blk.call(tool) }
      big_jpeg = "x" * (described_class::MIN_RENDER_BYTES + 1)
      allow(File).to receive(:binread).and_return(big_jpeg)

      part = described_class.new(document).send(:rasterize_page, "%PDF-1.4 fake", 1)

      expect(recorded.first.first).to eq(:density) # density must precede the input
      input = recorded.find { |k, v| k == :arg && v.to_s.include?(".pdf") }
      expect(input.last).to match(/\.pdf\[1\]\z/) # page via bracketed input arg
      expect(part).to include(type: :image, media_type: "image/jpeg")
    end

    it "treats a suspiciously small render as a failed page" do
      tool = double("convert", density: nil, quality: nil, "<<": nil)
      allow(MiniMagick).to receive(:convert) { |&blk| blk.call(tool) }
      allow(File).to receive(:binread).and_return("tiny")

      expect(described_class.new(document).send(:rasterize_page, "%PDF-1.4 fake", 0)).to be_nil
    end
  end
  describe "when no pages can be rasterized" do
    it "raises ParseError instead of sending an imageless prompt" do
      parser = described_class.new(document)
      allow(parser).to receive(:page_count).and_return(3)
      allow(parser).to receive(:rasterize_page).and_return(nil)

      expect { parser.call }.to raise_error(
        Reconciliations::ParseError, /couldn't render any pages/i
      )
      expect(adapter).not_to receive(:chat)
    end
  end

  describe "when the AI returns zero transactions" do
    it "raises ParseError instead of succeeding with an empty statement" do
      parser = described_class.new(document)
      allow(parser).to receive(:page_count).and_return(1)
      allow(parser).to receive(:rasterize_page)
        .and_return({ type: :image, media_type: "image/jpeg", data: "x" })
      allow(adapter).to receive(:chat)
        .and_return('{"currency":"EUR","bank_name":"KBC","transactions":[]}')

      expect { parser.call }.to raise_error(
        Reconciliations::ParseError, /couldn't read any transactions/i
      )
    end
  end

  describe "happy path" do
    it "returns the parsed hash when transactions are present" do
      parser = described_class.new(document)
      allow(parser).to receive(:page_count).and_return(1)
      allow(parser).to receive(:rasterize_page)
        .and_return({ type: :image, media_type: "image/jpeg", data: "x" })
      allow(adapter).to receive(:chat).and_return(
        '{"currency":"EUR","bank_name":"KBC","transactions":[{"date":"2024-06-03","description":"COMPRA X","amount":-12.5}]}'
      )

      result = parser.call
      expect(result["transactions"].length).to eq(1)
    end
  end
end
