class EmailAccount < ApplicationRecord
  include ProviderSyncDeactivation

  belongs_to :workspace

  encrypts :refresh_token

  # Harmonized OKLCH family (see app/assets/tailwind/application.css tones).
  COLORS = %w[#595dec #0584da #00a8a8 #2ea55c #dca81c #e76e08 #de3b3d #d44996].freeze

  enum :provider, { zoho: 0, google: 1, microsoft: 2 }, default: :zoho

  has_many :email_threads, dependent: :destroy
  has_many :email_messages, dependent: :restrict_with_error
  has_many :email_scan_logs, dependent: :restrict_with_error
  has_many :email_folders, dependent: :destroy
  has_many :external_tags, -> { external }, class_name: "Tag", dependent: :destroy
  has_many :email_account_users, dependent: :destroy
  has_many :users, through: :email_account_users
  has_many :email_account_signatures, dependent: :destroy
  has_many :signatures, through: :email_account_signatures

  validates :email_address, presence: true, uniqueness: true
  validates :refresh_token, presence: true
  validates :name, length: { maximum: 100 }, allow_blank: true

  before_create :assign_color

  # A scan that claimed the slot but never released it (worker killed mid-scan)
  # leaves `scanning = true` behind. After this window the claim is considered
  # stale: the next EmailScanJob run reclaims the slot *and* reconciles the flag
  # (EmailScanJob#reconcile_stale_scans), so the live sync pill can't get stuck
  # "perma-loading". Kept comfortably above the slowest real scan (seconds) while
  # short enough that an orphaned pill clears within a couple of poll cycles.
  SCAN_STALE_AFTER = 3.minutes

  scope :active, -> { where(active: true) }

  # Stable display order for the account pickers/filters (the inbox avatar strip
  # and the search filter panel). Without an explicit order these lists read back
  # in Postgres heap order, which shifts every time the sync jobs touch a row
  # (scanning flags, tokens, last_scanned_at) — so the filter kept reshuffling.
  # Sort by the visible label (the account name, falling back to its email
  # address) case-insensitively, then email as a stable tiebreaker.
  scope :ordered, -> { order(Arel.sql("LOWER(COALESCE(NULLIF(name, ''), email_address)) ASC, email_address ASC")) }

  # Accounts with a scan genuinely in flight: the flag is set *and* the claim is
  # still fresh. Excludes orphaned `scanning = true` rows from dead workers.
  scope :actively_scanning, -> { where(scanning: true).where(scan_started_at: SCAN_STALE_AFTER.ago..) }

  # Surface a disconnected account as an action-required notification, and clear
  # it automatically when the account is re-authenticated (active flips back).
  after_update :notify_disconnected,  if: -> { saved_change_to_active?(from: true, to: false) }
  after_update :resolve_disconnected, if: -> { saved_change_to_active?(from: false, to: true) }

  def deactivate!
    update!(active: false)
  end

  def record_scan!
    touch(:last_scanned_at)
  end

  # True when a scan is genuinely in flight for this account: the flag is set and
  # the claim is still fresh. The single-record mirror of the `actively_scanning`
  # scope, so an orphaned `scanning = true` from a dead worker doesn't read as live.
  def actively_scanning?
    scanning? && scan_started_at.present? && scan_started_at > SCAN_STALE_AFTER.ago
  end

  # Human-facing label for the account. Falls back to the email address when the
  # user hasn't given the account a custom name.
  def display_name
    name.presence || email_address
  end

  # Label for select menus and management rows, where the underlying mailbox
  # still matters even when a custom name is set: "Marketing (mkt@acme.com)".
  def select_label
    name.present? ? "#{name} (#{email_address})" : email_address
  end

  # Single-character initial for account avatars, derived from the display name.
  def avatar_initial
    display_name.strip.first.to_s.upcase
  end

  def mail_client
    case provider.to_sym
    when :google then Google::MailClient.new(self)
    when :microsoft then Microsoft::MailClient.new(self)
    else Zoho::MailClient.new(self)
    end
  end

  # Destination folders for this mailbox, normalized to { id:, name: }. Backs the
  # command palette's "move to folder" picker. Cached (provider API call); mirrors
  # the normalization in EmailMessagesController#folder_mappings.
  def folders
    Rails.cache.fetch("email_account/#{id}/folders", expires_in: 5.minutes) do
      list = mail_client.list_folders rescue []
      list.filter_map do |f|
        folder_id = f["folderId"]
        folder_name = f["folderName"]
        { id: folder_id, name: folder_name } if folder_id.present? && folder_name.present?
      end
    end
  end

  def oauth_client
    case provider.to_sym
    when :google then Google::OauthClient.new(refresh_token: refresh_token)
    when :microsoft then Microsoft::OauthClient.new(refresh_token: refresh_token)
    else Zoho::OauthClient.new(refresh_token: refresh_token)
    end
  end

  # The per-vendor delta-sync strategy (Gmail history / Graph delta / Zoho
  # windowed). EmailScanJob delegates the actual fetch here so the engine stays
  # provider-agnostic. See Emails::SyncStrategies.
  def sync_strategy
    Emails::SyncStrategies.for(self)
  end

  def accessible_by?(user)
    return false unless user
    email_account_users.exists?(user_id: user.id, can_read: true)
  end

  def sendable_by?(user)
    return false unless user
    email_account_users.exists?(user_id: user.id, can_send: true)
  end

  def managed_by?(user)
    return false unless user
    email_account_users.exists?(user_id: user.id, can_manage: true)
  end

  def owned_by?(user)
    return false unless user
    email_account_users.exists?(user_id: user.id, owner: true)
  end

  def permission_for(user)
    email_account_users.find_by(user_id: user.id)
  end

  private

  def notify_disconnected
    Notifier.account_disconnected(self)
  end

  def resolve_disconnected
    Notifier.account_reconnected(self)
  end

  def assign_color
    return if color.present? && color != COLORS.first && color != "#3b82f6"
    count = self.class.count
    self.color = COLORS[count % COLORS.size]
  end
end
