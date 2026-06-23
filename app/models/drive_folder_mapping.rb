class DriveFolderMapping < ApplicationRecord
  belongs_to :zoho_drive_account
  belongs_to :document_type, optional: true

  validates :drive_folder_id, presence: true
  validates :document_type_id, uniqueness: { scope: :zoho_drive_account_id }, allow_nil: true
end
