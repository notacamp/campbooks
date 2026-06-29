class FileShareLink < ApplicationRecord
  # A revocable, capability-based public link to a file (Document) or internal
  # document (AuthoredDocument). The unguessable token IS the credential — anyone
  # with the URL can open it without signing in (so it works for external email
  # recipients) until it's revoked or expires. Files Phase 3b.
  belongs_to :shareable, polymorphic: true
  belongs_to :created_by, class_name: "User", optional: true
  belongs_to :workspace

  has_secure_token :token

  scope :active, -> { where(revoked_at: nil) }
  scope :live, -> { active.where("expires_at IS NULL OR expires_at > ?", Time.current) }

  def revoked? = revoked_at.present?
  def expired? = expires_at.present? && expires_at <= Time.current
  def live? = !revoked? && !expired?

  def revoke!
    update!(revoked_at: Time.current)
  end

  def record_view!
    update_columns(view_count: view_count + 1, last_viewed_at: Time.current)
  end

  # Absolute URL safe to embed in outbound email / comments.
  def public_url(host:, protocol: nil)
    Rails.application.routes.url_helpers.public_file_url(token: token, host: host, protocol: protocol)
  end
end
