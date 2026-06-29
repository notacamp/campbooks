class MailFolder < ApplicationRecord
  belongs_to :workspace
  # Folders nest into a local tree (Campbooks-side organisation). Provider folders
  # stay flat — each folder still provisions/filters by its own unique name — so
  # nesting is purely how the pane groups them. Mirroring the tree onto provider
  # labels (e.g. Gmail "Parent/Child") is a future enhancement.
  belongs_to :parent, class_name: "MailFolder", optional: true
  has_many :children, -> { ordered }, class_name: "MailFolder", foreign_key: :parent_id,
                                      inverse_of: :parent, dependent: :nullify

  # Cap nesting depth so the tree (and any future provider label paths) stay sane.
  MAX_DEPTH = 3

  # Stage 3 "filesystem" layer — a folder can hold heterogeneous content via a
  # polymorphic join. Documents are wired first; emails stay provider-backed for now.
  has_many :folder_memberships, dependent: :destroy
  has_many :documents, through: :folder_memberships, source: :folderable, source_type: "Document"
  # Files Phase 2 — folders also hold authored (internal) documents and emails.
  has_many :authored_documents, through: :folder_memberships, source: :folderable, source_type: "AuthoredDocument"
  has_many :email_messages, through: :folder_memberships, source: :folderable, source_type: "EmailMessage"

  # Maps folder id → number of documents filed into it (for the pane badges).
  # One grouped query for the whole set, so the pane stays N+1-free.
  def self.document_counts(folders)
    ids = Array(folders).map(&:id)
    return {} if ids.empty?

    FolderMembership.where(mail_folder_id: ids, folderable_type: "Document").group(:mail_folder_id).count
  end

  # Maps folder id → total filed items of every kind (documents + internal docs +
  # emails) for the Files pane badges. One grouped query; N+1-free.
  def self.item_counts(folders)
    ids = Array(folders).map(&:id)
    return {} if ids.empty?

    FolderMembership.where(mail_folder_id: ids).group(:mail_folder_id).count
  end

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
  validate :parent_within_tree

  scope :ordered, -> { order(:position, :name) }
  scope :roots, -> { where(parent_id: nil) }

  # Next display position at the end of the workspace's chip strip.
  def self.next_position_for(workspace)
    (where(workspace: workspace).maximum(:position) || -1) + 1
  end

  # The icon name to draw for this folder's chip — the user's choice, or the
  # default folder glyph when unset.
  def display_icon
    icon.presence || Campbooks::Icon::DEFAULT
  end

  # Number of ancestors (a root folder is depth 0).
  def depth
    node = parent
    levels = 0
    while node
      levels += 1
      node = node.parent
    end
    levels
  end

  # This folder's id plus every descendant id — used to keep a folder from being
  # moved under itself and to offer only valid move targets in the UI.
  def self_and_descendant_ids
    ids = [ id ].compact
    queue = children.to_a
    until queue.empty?
      node = queue.shift
      ids << node.id
      queue.concat(node.children.to_a)
    end
    ids
  end

  private

  def name_not_reserved
    return if name.blank?

    errors.add(:name, :reserved) if RESERVED_NAMES.include?(name.downcase)
  end

  def parent_within_tree
    return if parent_id.blank?

    if parent_id == id
      errors.add(:parent, "can't be the folder itself")
    elsif parent && parent.workspace_id != workspace_id
      errors.add(:parent, "must be in the same workspace")
    elsif self_and_descendant_ids.include?(parent_id)
      errors.add(:parent, "can't be moved into one of its own subfolders")
    elsif parent && parent.depth >= MAX_DEPTH - 1
      errors.add(:parent, "would nest the folder too deeply")
    end
  end
end
