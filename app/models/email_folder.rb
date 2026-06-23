class EmailFolder < ApplicationRecord
  belongs_to :email_account

  validates :provider_folder_id, presence: true, uniqueness: { scope: :email_account_id }
  validates :name, presence: true
  validates :position, presence: true, numericality: { only_integer: true }

  scope :ordered, -> { order(:position) }

  DEFAULT_ORDER = %w[Inbox Sent Drafts Archive Spam Trash Snoozed Outbox Templates Newsletter Notification].freeze

  def self.default_position_for(name)
    idx = DEFAULT_ORDER.index(name)
    idx ? idx : DEFAULT_ORDER.size
  end
end
