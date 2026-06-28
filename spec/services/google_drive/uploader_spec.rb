require "rails_helper"

RSpec.describe GoogleDrive::Uploader do
  let(:workspace) { create(:workspace) }
  let(:document_type) { create(:document_type, workspace: workspace) }
  let(:account) { create(:google_drive_account, workspace: workspace, connected: true) }
  let(:config) { create(:google_drive_config, document_type: document_type, folder_id: "dest-folder") }
  let(:client) { instance_double(GoogleDrive::Client) }

  def fresh_doc
    create(:document, workspace: workspace, classification: document_type)
  end

  before do
    config && account
    allow(GoogleDrive::Client).to receive(:new).and_return(client)
    allow_any_instance_of(GoogleDrive::FolderResolver).to receive(:call).and_return("resolved-folder-id")
    allow_any_instance_of(GoogleDrive::FilenameBuilder).to receive(:call).and_return("20260627_ACME_Corp_INV-0042")
  end

  describe "#call" do
    it "uploads and sets status to pushed" do
      doc = fresh_doc
      result = double("drive_file", id: "f-999", name: "doc.pdf", web_view_link: "https://example.com")
      allow(client).to receive(:upload_file).with(any_args).and_return(result)

      described_class.new(doc).call
      doc.reload

      expect(doc.drive_pushed?).to be(true)
      expect(doc.google_drive_file_id).to eq("f-999")
      expect(doc.google_drive_pushed_at).to be_present
    end

    it "marks failed and re-raises on upload error" do
      doc = fresh_doc
      allow(client).to receive(:upload_file).with(any_args).and_raise(GoogleDrive::ApiError.new("Upload failed"))

      expect { described_class.new(doc).call }.to raise_error(GoogleDrive::ApiError)
      doc.reload
      expect(doc.drive_failed?).to be(true)
      expect(doc.google_drive_push_error).to include("Upload failed")
    end

    it "raises when there is no config" do
      config.destroy!
      document_type.reload
      doc = fresh_doc
      doc.original_file.purge

      expect { described_class.new(doc).call }.to raise_error(/No Google Drive config/)
    end

    it "raises when account is not connected" do
      account.update!(connected: false)
      doc = fresh_doc

      expect { described_class.new(doc).call }.to raise_error(/Google Drive not connected/)
    end

    it "raises when there is no attached file" do
      doc = fresh_doc
      doc.original_file.purge

      expect { described_class.new(doc).call }.to raise_error(/No file to upload/)
    end
  end
end
