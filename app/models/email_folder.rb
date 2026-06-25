class EmailFolder < ApplicationRecord
  belongs_to :email_account

  validates :provider_folder_id, presence: true, uniqueness: { scope: :email_account_id }
  validates :name, presence: true
  validates :position, presence: true, numericality: { only_integer: true }

  scope :ordered, -> { order(:position) }

  DEFAULT_ORDER = %w[Inbox Sent Drafts Archive Spam Trash Snoozed Outbox Templates Newsletter Notification].freeze

  # Outbound/compose system folders that must never accept moved mail — dropping
  # a received message into Sent or Drafts is nonsensical, so the inbox folder
  # chips for these are not drag-and-drop / tap-to-move targets.
  UNDROPPABLE_NAMES = %w[Sent Drafts].freeze

  def self.droppable_name?(name)
    UNDROPPABLE_NAMES.exclude?(name.to_s)
  end

  def self.default_position_for(name)
    idx = DEFAULT_ORDER.index(name)
    idx ? idx : DEFAULT_ORDER.size
  end
end
