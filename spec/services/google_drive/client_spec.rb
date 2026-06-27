require "rails_helper"

RSpec.describe GoogleDrive::Client do
  let(:account) { build_stubbed(:google_drive_account, connected: true) }
  let(:oauth_client) { instance_double(GoogleDrive::OauthClient, refresh_access_token: "access-token") }
  let(:faraday_connection) { instance_double(Faraday::Connection) }

  before do
    allow(GoogleDrive::OauthClient).to receive(:new).and_return(oauth_client)
    allow(Faraday).to receive(:new).and_return(faraday_connection)
    allow(faraday_connection).to receive(:headers)
    allow(faraday_connection).to receive(:options).and_return(
      double("options", open_timeout: 10, timeout: 30, open_timeout=: nil, timeout=: nil)
    )
  end

  describe "#upload_file" do
    let(:tempfile) { Tempfile.new(["test", ".pdf"]) }

    before { tempfile.write("fake pdf content"); tempfile.rewind }
    after  { tempfile.close! }

    it "uploads via multipart and returns id/name/web_view_link" do
      body = { id: "file-123", name: "test.pdf", webViewLink: "https://drive.google.com/file/d/file-123/view" }.to_json
      allow(faraday_connection).to receive(:post).and_return(double("r", body: body, status: 200))

      result = described_class.new(account).upload_file(file_path: tempfile.path, file_name: "test.pdf", mime_type: "application/pdf")
      expect(result.id).to eq("file-123")
      expect(result.web_view_link).to include("drive.google.com")
    end

    it "raises ApiError on failure" do
      body = { error: { message: "Insufficient permissions" } }.to_json
      allow(faraday_connection).to receive(:post).and_return(double("r", body: body, status: 403))

      expect { described_class.new(account).upload_file(file_path: tempfile.path, file_name: "t.pdf", mime_type: "application/pdf") }
        .to raise_error(GoogleDrive::ApiError, /Insufficient permissions/)
    end

    it "raises when account is not connected" do
      account.connected = false
      expect { described_class.new(account).upload_file(file_path: tempfile.path, file_name: "t.pdf", mime_type: "application/pdf") }
        .to raise_error(/not connected/)
    end
  end

  describe "#create_folder" do
    it "creates a folder and returns id/name" do
      body = { id: "f-789", name: "Invoices" }.to_json
      allow(faraday_connection).to receive(:post).and_return(double("r", body: body, status: 200))

      result = described_class.new(account).create_folder("Invoices")
      expect(result.id).to eq("f-789")
      expect(result.name).to eq("Invoices")
    end
  end

  describe "#find_folder_by_name" do
    it "returns the folder when found" do
      body = { files: [{ id: "f-1", name: "Invoices" }] }.to_json
      allow(faraday_connection).to receive(:get).and_return(double("r", body: body, status: 200))

      result = described_class.new(account).find_folder_by_name("Invoices")
      expect(result.id).to eq("f-1")
    end

    it "returns nil when not found" do
      allow(faraday_connection).to receive(:get).and_return(double("r", body: { files: [] }.to_json, status: 200))
      expect(described_class.new(account).find_folder_by_name("Nope")).to be_nil
    end
  end

  describe "#list_folders" do
    it "lists child folders as OpenStructs" do
      body = { files: [{ id: "f-1", name: "Finance" }, { id: "f-2", name: "HR" }] }.to_json
      allow(faraday_connection).to receive(:get).and_return(double("r", body: body, status: 200))

      results = described_class.new(account).list_folders
      expect(results.size).to eq(2)
      expect(results.first.name).to eq("Finance")
    end

    it "raises ApiError on failure" do
      body = { error: { message: "Not found" } }.to_json
      allow(faraday_connection).to receive(:get).and_return(double("r", body: body, status: 404))

      expect { described_class.new(account).list_folders }.to raise_error(GoogleDrive::ApiError)
    end
  end

  describe "#get_folder" do
    it "fetches a single folder by id" do
      body = { id: "f-42", name: "My Folder", mimeType: "application/vnd.google-apps.folder" }.to_json
      allow(faraday_connection).to receive(:get).and_return(double("r", body: body, status: 200))

      result = described_class.new(account).get_folder("f-42")
      expect(result.name).to eq("My Folder")
    end

    it "returns nil on API error" do
      allow(faraday_connection).to receive(:get).and_return(double("r", body: { error: { message: "X" } }.to_json, status: 404))
      expect(described_class.new(account).get_folder("nope")).to be_nil
    end

    it "returns nil on Faraday error" do
      allow(faraday_connection).to receive(:get).and_raise(Faraday::TimeoutError.new("timeout"))
      expect(described_class.new(account).get_folder("any")).to be_nil
    end
  end

  describe "#find_or_create_folder" do
    it "creates missing segments and returns the final folder id" do
      allow(faraday_connection).to receive(:get).and_return(
        double("r", body: { files: [] }.to_json, status: 200),
        double("r", body: { files: [] }.to_json, status: 200)
      )
      allow(faraday_connection).to receive(:post).and_return(
        double("r", body: { id: "yr-2026", name: "2026" }.to_json, status: 200),
        double("r", body: { id: "mo-06", name: "06_June" }.to_json, status: 200)
      )

      result = described_class.new(account).find_or_create_folder(%w[2026 06_June], root_folder_id: "root")
      expect(result).to eq("mo-06")
    end

    it "reuses existing folders" do
      allow(faraday_connection).to receive(:get).and_return(
        double("r", body: { files: [{ id: "existing", name: "2026" }] }.to_json, status: 200)
      )

      result = described_class.new(account).find_or_create_folder(%w[2026], root_folder_id: "root")
      expect(result).to eq("existing")
    end
  end
end
