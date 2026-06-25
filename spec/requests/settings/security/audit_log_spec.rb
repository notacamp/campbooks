require "rails_helper"

RSpec.describe "Settings::Security::AuditLog", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  describe "GET /settings/security/audit_log" do
    it "renders the user's own security events" do
      AuditEvent.create!(user: user, action: "password_changed")

      get audit_log_settings_security_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("settings.security.audit_log.actions.password_changed"))
    end

    it "shows the current user's events but not another user's" do
      other = create(:user)
      AuditEvent.create!(user: other, action: "data_exported")
      AuditEvent.create!(user: user, action: "mfa_totp_enabled")

      get audit_log_settings_security_path

      expect(response.body).to include(I18n.t("settings.security.audit_log.actions.mfa_totp_enabled"))
      expect(response.body).not_to include(I18n.t("settings.security.audit_log.actions.data_exported"))
    end
  end
end
