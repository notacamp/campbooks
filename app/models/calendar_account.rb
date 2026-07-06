class CalendarAccount < ApplicationRecord
  include ProviderSyncDeactivation

  belongs_to :workspace

  encrypts :refresh_token

  # Shared OKLCH palette (mirrors EmailAccount::COLORS).
  COLORS = %w[#595dec #0584da #00a8a8 #2ea55c #dca81c #e76e08 #de3b3d #d44996].freeze

  enum :provider, { google: 0, zoho: 1 }, default: :google

  has_many :calendars, dependent: :destroy
  has_many :calendar_events, through: :calendars
  has_many :calendar_sync_logs, dependent: :restrict_with_error
  has_many :calendar_account_users, dependent: :destroy
  has_many :users, through: :calendar_account_users

  # The same login can be linked once per provider (see the (email, provider)
  # unique index), so uniqueness is scoped to provider rather than global.
  validates :email_address, presence: true, uniqueness: { scope: :provider }
  validates :refresh_token, presence: true
  validates :name, length: { maximum: 100 }, allow_blank: true

  before_create :assign_color

  # Slot-lock staleness window — see EmailAccount::SCAN_STALE_AFTER.
  SCAN_STALE_AFTER = 3.minutes

  scope :active, -> { where(active: true) }
  scope :actively_scanning, -> { where(scanning: true).where(scan_started_at: SCAN_STALE_AFTER.ago..) }

  # TODO(polish): proactive disconnect/reconnect notifications, mirroring
  # EmailAccount's after_update callbacks — needs calendar-specific Notifier
  # methods + i18n (link to the calendar settings page, not the inbox).

  def deactivate!
    update!(active: false)
  end

  def actively_scanning?
    scanning? && scan_started_at.present? && scan_started_at > SCAN_STALE_AFTER.ago
  end

  def display_name
    name.presence || email_address
  end

  def select_label
    name.present? ? "#{name} (#{email_address})" : email_address
  end

  def avatar_initial
    display_name.strip.first.to_s.upcase
  end

  def calendar_client
    case provider.to_sym
    when :zoho then Zoho::CalendarClient.new(self)
    else Google::CalendarClient.new(self)
    end
  end

  # The calendar rides on the mailbox's OAuth grant (combined mail+calendar
  # scopes), so it refreshes through the same client and shared token cache.
  def oauth_client
    case provider.to_sym
    when :zoho then Zoho::OauthClient.new(refresh_token: refresh_token)
    else Google::OauthClient.new(refresh_token: refresh_token)
    end
  end

  def accessible_by?(user)
    return false unless user
    calendar_account_users.exists?(user_id: user.id, can_read: true)
  end

  def writable_by?(user)
    return false unless user
    calendar_account_users.exists?(user_id: user.id, can_write: true)
  end

  def managed_by?(user)
    return false unless user
    calendar_account_users.exists?(user_id: user.id, can_manage: true)
  end

  def owned_by?(user)
    return false unless user
    calendar_account_users.exists?(user_id: user.id, owner: true)
  end

  def permission_for(user)
    calendar_account_users.find_by(user_id: user.id)
  end

  private

  def assign_color
    return if color.present? && color != COLORS.first && color != "#3b82f6"
    count = self.class.count
    self.color = COLORS[count % COLORS.size]
  end
end
