class Session < ApplicationRecord
  belongs_to :user

  # The signed session cookie is permanent, so this server-side inactivity window
  # is what actually bounds a login — and it stops ip_address/user_agent rows from
  # being retained indefinitely (GDPR storage limitation, Art. 5(1)(e)). Expired
  # sessions are rejected on resume (Authentication#find_session_by_cookie) and
  # swept by SessionsPruneJob.
  INACTIVITY_LIMIT = 30.days

  scope :expired, -> { where(updated_at: ..INACTIVITY_LIMIT.ago) }

  def expired?
    updated_at < INACTIVITY_LIMIT.ago
  end

  # Slide the inactivity window forward on use, but at most once a day so we don't
  # write to the row on every request.
  def touch_if_stale
    touch if updated_at < 1.day.ago
  end

  # ── First-party app API (/api/app) bearer token ─────────────────────────────
  # The React SPA + Capacitor app authenticate with a bearer token instead of the
  # signed cookie. The token is this row's signed_id: tamper-proof, carries the
  # session id (so ip_address/user_agent/inactivity all still apply), needs no
  # schema change, and is revoked by destroying the row. See api-migration/00-auth.md.
  API_TOKEN_PURPOSE = "api_app"

  def api_token
    signed_id(purpose: API_TOKEN_PURPOSE)
  end

  # Resolve a bearer token back to its (live, non-expired) Session, or nil. Slides
  # the inactivity window forward on use, exactly like the cookie resume path.
  def self.authenticate_api_token(token)
    return if token.blank?

    session = find_signed(token, purpose: API_TOKEN_PURPOSE)
    return if session.nil? || session.expired?

    session.touch_if_stale
    session
  end
end
