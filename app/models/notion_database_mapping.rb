class NotionDatabaseMapping < ApplicationRecord
  belongs_to :document_type
  has_many :notion_pages, dependent: :restrict_with_error

  validates :notion_database_id, presence: true
  validates :document_type_id, uniqueness: true
end
