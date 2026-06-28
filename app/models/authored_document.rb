class AuthoredDocument < ApplicationRecord
  belongs_to :workspace
  belongs_to :author, class_name: "User", optional: true

  validates :title, presence: true, length: { maximum: 255 }
  validates :html_content, length: { maximum: 500_000 }

  scope :recent, -> { order(created_at: :desc) }
end
