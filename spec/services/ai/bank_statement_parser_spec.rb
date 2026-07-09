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

  describe "page rasterization (regression: prod 2026-07-09)" do
    # MiniMagick 5 validates the path handed to .open, so the ImageMagick
    # bracket syntax ("file.pdf[0]") raises ENOENT. The page must be selected
    # via #page AFTER opening. This spec drives the real rasterize_page method
    # with a mocked MiniMagick and fails if the bracket syntax comes back.
    it "opens the tempfile path without bracket page syntax and selects the page via #page" do
      image = double("MiniMagick::Image", path: "/tmp/converted.jpg")
      opened_paths = []

      allow(MiniMagick::Image).to receive(:open) do |path|
        opened_paths << path
        raise Errno::ENOENT, path if path.include?("[")

        image
      end
      allow(image).to receive(:format)
      allow(image).to receive(:density)
      allow(image).to receive(:page)
      allow(File).to receive(:binread).with("/tmp/converted.jpg").and_return("jpegbytes")

      part = described_class.new(document).send(:rasterize_page, "%PDF-1.4 fake", 2)

      expect(opened_paths).to all(satisfy { |p| !p.include?("[") })
      expect(image).to have_received(:page).with("2")
      expect(part).to include(type: :image, media_type: "image/jpeg")
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
