require "rails_helper"

RSpec.describe Notion::FileUploader do
  describe ".files_property" do
    it "builds a files property referencing uploaded ids" do
      prop = described_class.files_property(%w[fu_1 fu_2], names: %w[a.pdf b.pdf])

      expect(prop["files"].size).to eq(2)
      expect(prop["files"][0]).to include("type" => "file_upload", "name" => "a.pdf")
      expect(prop["files"][0].dig("file_upload", "id")).to eq("fu_1")
    end

    it "falls back to a default name when none is given" do
      prop = described_class.files_property([ "fu_1" ])
      expect(prop["files"][0]["name"]).to eq("file")
    end
  end

  describe ".file_block" do
    it "builds a file block for page children" do
      block = described_class.file_block("fu_9", name: "report.pdf")
      expect(block["type"]).to eq("file")
      expect(block.dig("file", "file_upload", "id")).to eq("fu_9")
      expect(block.dig("file", "name")).to eq("report.pdf")
    end
  end

  describe "#upload routing" do
    let(:integration) { double("NotionIntegration", access_token: "tok") }
    subject(:uploader) { described_class.new(integration) }

    before do
      allow(uploader).to receive(:create_file_upload).and_return({ "id" => "fu_x" })
      allow(uploader).to receive(:send_part).and_return({ "status" => "uploaded" })
      allow(uploader).to receive(:complete_file_upload).and_return({ "status" => "uploaded" })
    end

    it "uses the multi-part flow (chunks + complete) for files over 20 MB" do
      big = StringIO.new("x" * (described_class::MAX_SINGLE_PART_BYTES + 5))

      expect(uploader.upload(io: big, filename: "big.bin")).to eq("fu_x")
      expect(uploader).to have_received(:complete_file_upload).with("fu_x")
      expect(uploader).to have_received(:send_part).at_least(:twice) # at least two chunks
    end

    it "uses the single-part flow (no complete) for small files" do
      small = StringIO.new("hello")

      expect(uploader.upload(io: small, filename: "small.txt")).to eq("fu_x")
      expect(uploader).to have_received(:send_part).once
      expect(uploader).not_to have_received(:complete_file_upload)
    end
  end
end
