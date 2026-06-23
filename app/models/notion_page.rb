class NotionPage < ApplicationRecord
  belongs_to :document
  belongs_to :notion_database_mapping

  enum :sync_status, { synced: 0, outdated: 1, error: 2 }

  validates :notion_page_id, presence: true, uniqueness: true
  validates :document_id, uniqueness: true
end
