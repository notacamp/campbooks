class CalendarSyncLog < ApplicationRecord
  belongs_to :calendar_account

  enum :status, { running: 0, completed: 1, failed: 2 }

  validates :status, presence: true
end
