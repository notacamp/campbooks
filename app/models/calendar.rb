class Calendar < ApplicationRecord
  belongs_to :calendar_account
  has_many :calendar_events, dependent: :destroy
  has_many :calendar_webhook_channels, dependent: :destroy

  validates :provider_calendar_id, presence: true, uniqueness: { scope: :calendar_account_id }
  validates :name, presence: true

  # Only calendars the user toggled on are pulled by CalendarScanJob.
  scope :syncing, -> { where(syncing: true) }

  delegate :workspace, :provider, to: :calendar_account

  # User-facing color, falling back to the account color when the provider
  # didn't supply one for this calendar.
  def display_color
    color.presence || calendar_account.color
  end
end
