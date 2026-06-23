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
end
