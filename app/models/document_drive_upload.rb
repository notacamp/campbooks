class DocumentDriveUpload < ApplicationRecord
  belongs_to :document
  belongs_to :zoho_drive_account

  validates :document_id, uniqueness: { scope: :zoho_drive_account_id }

  scope :pending, -> { where(status: "pending") }
  scope :uploaded, -> { where(status: "uploaded") }
  scope :failed, -> { where(status: "failed") }
end
