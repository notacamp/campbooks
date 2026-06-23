class CalendarWebhookChannel < ApplicationRecord
  belongs_to :calendar

  validates :provider_channel_id, presence: true, uniqueness: true
  validates :channel_token, presence: true

  scope :expiring_before, ->(time) { where(expires_at: ..time) }

  delegate :calendar_account, to: :calendar

  def expired?
    expires_at.present? && expires_at <= Time.current
  end
end
