class ZohoDriveAccount < ApplicationRecord
  encrypts :zoho_refresh_token

  belongs_to :workspace
  has_many :drive_folder_mappings, dependent: :destroy
  has_many :document_drive_uploads, dependent: :restrict_with_error

  validates :email_address, presence: true, uniqueness: { scope: :workspace_id }
  validates :zoho_refresh_token, presence: true

  scope :active, -> { where(active: true) }

  def deactivate!
    update!(active: false)
  end

  def record_sync!
    touch(:last_synced_at)
  end
end
