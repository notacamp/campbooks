# A saved outbound HTTP integration (base URL + auth) that workflow
# `custom_action` steps call. Auth is resolved server-side at run time
# (see #auth_headers), so a secret is never stored or rendered in a step's
# plaintext Liquid config. Workspace-scoped and surfaced under
# Settings → Integrations.
class Connection < ApplicationRecord
  belongs_to :workspace

  encrypts :auth_secret

  AUTH_TYPES = %w[none bearer header basic].freeze

  validates :name, presence: true
  validates :base_url, presence: true
  validates :auth_type, inclusion: { in: AUTH_TYPES }
  validates :auth_secret, presence: true, unless: -> { auth_type == "none" }
  validates :auth_header_name, presence: true, if: -> { auth_type == "header" }
  # Reject CRLF / illegal characters so a stored header name can't smuggle extra
  # headers into the outbound request (RFC 7230 header-name token charset).
  validates :auth_header_name, format: { with: /\A[A-Za-z0-9!#$%&'*+.^_`|~-]+\z/ }, allow_blank: true
  validate :base_url_is_http

  normalizes :base_url, with: ->(value) { value.to_s.strip.chomp("/") }

  scope :ordered, -> { order(:name) }

  # Header(s) to merge into an outbound request to authenticate as this
  # connection. Empty for `none`. The workflow executor merges these over the
  # step's own headers so a step can never override the credential.
  def auth_headers
    case auth_type
    when "bearer"
      { "Authorization" => "Bearer #{auth_secret}" }
    when "header"
      auth_header_name.present? ? { auth_header_name => auth_secret.to_s } : {}
    when "basic"
      { "Authorization" => "Basic #{Base64.strict_encode64("#{auth_username}:#{auth_secret}")}" }
    else
      {}
    end
  end

  def select_label
    name
  end

  private

  def base_url_is_http
    return if base_url.blank?

    uri = URI.parse(base_url)
    unless uri.is_a?(URI::HTTP) && uri.host.present?
      errors.add(:base_url, "must be an http(s) URL")
    end
  rescue URI::InvalidURIError
    errors.add(:base_url, "is not a valid URL")
  end
end
