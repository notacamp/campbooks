require "rails_helper"

RSpec.describe AuditEvent do
  let(:user) { create(:user) }

  describe ".log" do
    it "records an immutable audit event for an action" do
      event = AuditEvent.log("password_changed", user: user)
      expect(event).to be_persisted
      expect(event.action).to eq("password_changed")
      expect(event.user).to eq(user)
    end

    it "captures ip / user-agent from the request, plus a target and metadata" do
      request = instance_double(ActionDispatch::Request, remote_ip: "203.0.113.9", user_agent: "Firefox")
      event = AuditEvent.log("admin_role_changed", user: user, request: request, target: user, role: "admin")
      expect(event.ip_address).to eq("203.0.113.9")
      expect(event.user_agent).to eq("Firefox")
      expect(event.target).to eq(user)
      expect(event.metadata).to eq({ "role" => "admin" })
    end

    it "is best-effort — never raises, returns nil on failure" do
      allow(AuditEvent).to receive(:create!).and_raise(ActiveRecord::StatementInvalid, "boom")
      result = nil
      expect { result = AuditEvent.log("sign_in", user: user) }.not_to raise_error
      expect(result).to be_nil
    end
  end

  it "survives (anonymised) when its user is deleted — FK nullifies rather than blocking erasure" do
    event = AuditEvent.log("sign_in", user: user)
    user.destroy!
    expect(event.reload.user_id).to be_nil
  end
end
