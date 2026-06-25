require "rails_helper"

RSpec.describe "Settings::Account export", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  describe "POST /settings/account/export" do
    it "creates a pending export, enqueues the job, and redirects" do
      expect {
        post export_settings_account_path
      }.to change { user.account_exports.count }.by(1)
        .and have_enqueued_job(AccountExportJob)

      expect(response).to redirect_to(settings_account_path)
      expect(user.account_exports.last).to be_pending
    end

    it "logs a data_exported audit event" do
      expect { post export_settings_account_path }
        .to change { AuditEvent.where(action: "data_exported", user: user).count }.by(1)
    end
  end

  describe "GET /settings/account/download_export" do
    it "redirects back with an alert when no archive is ready" do
      get download_export_settings_account_path
      expect(response).to redirect_to(settings_account_path)
    end

    it "serves the latest generated archive" do
      account_export = user.account_exports.create!(status: :generated)
      account_export.archive.attach(io: StringIO.new("zip"), filename: "export.zip", content_type: "application/zip")

      get download_export_settings_account_path
      expect(response).to have_http_status(:redirect)
      expect(response.location).to include("export.zip")
    end
  end
end
