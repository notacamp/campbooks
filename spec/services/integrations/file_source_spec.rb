require "rails_helper"

RSpec.describe Integrations::FileSource do
  describe ".for" do
    it "yields the document's original file as a descriptor" do
      document = create(:document)

      files = described_class.for(document: document)

      expect(files.size).to eq(1)
      expect(files.first.filename).to eq("invoice.pdf")
      expect(files.first.content_type).to eq("application/pdf")
      files.first.open { |io| expect(io.read).to eq("fake pdf content") }
    end

    it "returns nothing for a document without a file" do
      document = create(:document)
      document.original_file.purge

      expect(described_class.for(document: document)).to be_empty
    end

    it "yields an email's attachments, filterable by blob id" do
      message = create(:email_message)
      message.files.attach(io: StringIO.new("one"), filename: "one.txt", content_type: "text/plain")
      message.files.attach(io: StringIO.new("two"), filename: "two.txt", content_type: "text/plain")

      all = described_class.for(email_message: message)
      expect(all.map(&:filename)).to match_array(%w[one.txt two.txt])

      first_blob_id = message.files.blobs.first.id
      subset = described_class.for(email_message: message, blob_ids: [ first_blob_id ])
      expect(subset.size).to eq(1)
    end

    it "returns nothing when neither context is given" do
      expect(described_class.for).to eq([])
    end
  end
end
