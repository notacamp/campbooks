require "rails_helper"

RSpec.describe "Audit logging", type: :request do
  it "logs a sign_in event when a user signs in" do
    user = create(:user)
    expect { sign_in(user) }.to change { AuditEvent.where(action: "sign_in", user: user).count }.by(1)
  end

  it "logs a data_exported event when the user requests their data" do
    user = create(:user)
    sign_in(user)
    expect { post export_settings_account_path }
      .to change { AuditEvent.where(action: "data_exported", user: user).count }.by(1)
  end
end
