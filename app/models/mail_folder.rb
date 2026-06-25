class MailFolder < ApplicationRecord
  belongs_to :workspace

  # A user-defined folder shown as a chip on top of the inbox. Creating one
  # provisions a real provider folder (or Gmail label) on every connected
  # account — see MailFolders::Provisioner. This record is the canonical,
  # account-independent identity; the per-account provider folders live in the
  # `email_folders` mirror, joined back by name.

  # Custom folders must not shadow system/provider folders — the chip bar and
  # name-based filtering both key on the name, so "Inbox" etc. would be ambiguous.
  # EmailFolder::DEFAULT_ORDER is a superset of the baseline system folders.
  RESERVED_NAMES = EmailFolder::DEFAULT_ORDER.map(&:downcase).freeze

  normalizes :name, with: ->(value) { value.to_s.strip }

  validates :name, presence: true, length: { maximum: 100 }
  validates :name, uniqueness: { scope: :workspace_id, case_sensitive: false }
  validate :name_not_reserved
  validates :position, numericality: { only_integer: true }
  # Icon is optional (blank → the default folder glyph). The lambda defers loading
  # the Campbooks::Icon component until validation time, so the model doesn't pull
  # the view layer in at class-load.
  validates :icon, inclusion: { in: ->(_) { Campbooks::Icon::NAMES } }, allow_blank: true

  scope :ordered, -> { order(:position, :name) }

  # Next display position at the end of the workspace's chip strip.
  def self.next_position_for(workspace)
    (where(workspace: workspace).maximum(:position) || -1) + 1
  end

  # The icon name to draw for this folder's chip — the user's choice, or the
  # default folder glyph when unset.
  def display_icon
    icon.presence || Campbooks::Icon::DEFAULT
  end

  private

  def name_not_reserved
    return if name.blank?

    errors.add(:name, :reserved) if RESERVED_NAMES.include?(name.downcase)
  end
end
