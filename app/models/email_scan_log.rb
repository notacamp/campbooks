class EmailScanLog < ApplicationRecord
  belongs_to :email_account
  has_many :email_messages, dependent: :restrict_with_error

  enum :status, {
    running: 0,
    completed: 1,
    failed: 2
  }

  validates :status, presence: true
end
