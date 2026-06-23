class Export < ApplicationRecord
  belongs_to :workspace

  has_one_attached :zip_file

  enum :status, {
    pending: 0,
    generating: 1,
    generated: 2,
    failed: 3
  }

  validates :status, presence: true

  scope :recent, -> { order(created_at: :desc) }
end
