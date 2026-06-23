class AuditEvent < ApplicationRecord
  belongs_to :user, optional: true
  belongs_to :target, polymorphic: true, optional: true

  validates :action, presence: true

  # Append an immutable audit record for an accountability-relevant action (GDPR
  # Art. 5(2) / 32). Best-effort: a logging failure must never break the request
  # it is recording, so errors are caught and swallowed.
  def self.log(action, user: nil, request: nil, target: nil, **metadata)
    create!(
      action: action.to_s,
      user: user,
      target: target,
      ip_address: request&.remote_ip,
      user_agent: request&.user_agent,
      metadata: metadata
    )
  rescue StandardError => e
    Rails.logger.warn("[AuditEvent] failed to log #{action}: #{e.class}: #{e.message}")
    nil
  end
end
