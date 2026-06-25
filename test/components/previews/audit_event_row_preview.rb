# frozen_string_literal: true

# Previews for the personal security/audit-log row. In-memory (unsaved)
# AuditEvent records stand in for real rows, covering the sign-in, MFA,
# linked-provider, data-export, and account-deletion icon/label variants.
class AuditEventRowPreview < ViewComponent::Preview
  def sign_in
    render Campbooks::AuditEventRow.new(event: event("sign_in",
      ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605.1.15"))
  end

  def mfa_enabled
    render Campbooks::AuditEventRow.new(event: event("mfa_totp_enabled"))
  end

  def sign_in_method_added
    render Campbooks::AuditEventRow.new(event: event("sign_in_method_added",
      metadata: { "provider" => "google" }))
  end

  def data_exported
    render Campbooks::AuditEventRow.new(event: event("data_exported"))
  end

  def account_deletion_requested
    render Campbooks::AuditEventRow.new(event: event("account_deletion_requested"))
  end

  private

  def event(action, metadata: {}, ua: "Mozilla/5.0")
    AuditEvent.new(
      action: action,
      ip_address: "203.0.113.7",
      user_agent: ua,
      metadata: metadata,
      created_at: 2.hours.ago
    )
  end
end
