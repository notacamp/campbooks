class GoogleDriveConfig < ApplicationRecord
  belongs_to :document_type

  enum :subfolder_pattern, {
    flat: 0,
    year: 1,
    year_month: 2,
    entity: 3
  }

  validates :document_type_id, uniqueness: true
  validates :naming_pattern, presence: true
  validates :folder_id, presence: true, if: :auto_push?
end
