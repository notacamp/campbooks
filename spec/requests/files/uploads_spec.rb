require "rails_helper"

RSpec.describe "Files::Uploads", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  def upload(name = "notes.txt", type = "text/plain")
    Rack::Test::UploadedFile.new(StringIO.new("hello world"), type, original_filename: name)
  end

  describe "POST /files/uploads" do
    it "stores the file as a manual upload without running the AI pipeline" do
      expect do
        post files_uploads_path, params: { files: [ upload ] }
      end.to change(workspace.documents, :count).by(1)

      doc = workspace.documents.order(:created_at).last
      expect(doc.source).to eq("manual_upload")
      expect(doc.ai_status).to eq("skipped")
      expect(response).to redirect_to(files_path)
    end

    it "does not enqueue DocumentProcessJob (light path)" do
      expect(DocumentProcessJob).not_to receive(:perform_later)
      post files_uploads_path, params: { files: [ upload ] }
    end

    it "publishes a file.uploaded event" do
      expect do
        post files_uploads_path, params: { files: [ upload ] }
      end.to change { workspace.events.where(name: "file.uploaded").count }.by(1)
    end

    it "files the upload into a folder when folder_id is given" do
      folder = create(:mail_folder, workspace: workspace)

      post files_uploads_path, params: { files: [ upload ], folder_id: folder.id }

      doc = workspace.documents.order(:created_at).last
      expect(folder.reload.documents).to include(doc)
      expect(response).to redirect_to(files_folder_path(folder))
    end

    it "redirects with an error when no files are chosen" do
      post files_uploads_path, params: { files: [] }
      expect(response).to redirect_to(files_path)
    end
  end

  describe "DELETE /files/uploads/:id" do
    it "deletes a manual upload" do
      doc = create(:document, :other, workspace: workspace, source: :manual_upload)
      expect do
        delete files_upload_path(doc)
      end.to change(workspace.documents, :count).by(-1)
    end

    it "404s for an email-sourced document (managed by the mail pipeline)" do
      doc = create(:document, :other, workspace: workspace, source: :email)
      delete files_upload_path(doc)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /files/uploads/:id/analyze" do
    it "flips the file to pending and enqueues analysis when AI is available" do
      allow_any_instance_of(Files::UploadsController).to receive(:ai_provider_available?).and_return(true)
      doc = create(:document, :other, workspace: workspace, source: :manual_upload, ai_status: :skipped)

      expect(DocumentProcessJob).to receive(:perform_later).with(doc.id)
      post analyze_files_upload_path(doc)

      expect(doc.reload.ai_status).to eq("pending")
    end

    it "does not analyze when no AI provider is configured" do
      allow_any_instance_of(Files::UploadsController).to receive(:ai_provider_available?).and_return(false)
      doc = create(:document, :other, workspace: workspace, source: :manual_upload, ai_status: :skipped)

      expect(DocumentProcessJob).not_to receive(:perform_later)
      post analyze_files_upload_path(doc)

      expect(doc.reload.ai_status).to eq("skipped")
    end
  end
end
