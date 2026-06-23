class GoogleDriveAccount < ApplicationRecord
  belongs_to :workspace

  encrypts :refresh_token

  validates :refresh_token, presence: true
  validates :email, presence: true, if: :connected?

  scope :connected, -> { where(connected: true) }

  def deactivate!
    update!(connected: false)
  end

  # True when the account was authorized with the full Drive scope (folder browsing).
  # Accounts connected before the scope upgrade hold only `drive.file` and must
  # reconnect; nil scopes (pre-migration rows) are treated as legacy.
  def full_access?
    scopes.to_s.split.include?(GoogleDrive::OauthClient::FULL_SCOPE)
  end
end
