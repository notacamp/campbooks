class Tag < ApplicationRecord
  has_rich_text :prompt

  # Return plain text (markdown) from the rich text body instead of HTML.
  def prompt
    rich_text_prompt&.body&.to_plain_text.presence
  end

  # ActionText's generated `prompt=` reads back through the overridden getter
  # above (which returns a String, not the RichText record), so it blows up with
  # "undefined method `body='". Write to the association directly instead.
  def prompt=(value)
    (rich_text_prompt || build_rich_text_prompt).body = value
  end

  belongs_to :workspace
  belongs_to :email_account, optional: true

  has_one :search_tag_embedding, dependent: :destroy

  has_many :email_message_tags, dependent: :destroy
  has_many :email_messages, through: :email_message_tags
  has_many :task_tags, dependent: :destroy
  has_many :tasks, through: :task_tags
  has_many :notification_preferences, dependent: :destroy

  after_save_commit :enqueue_tag_embedding

  enum :source, { local: 0, external: 1 }

  # Exclude system labels (INBOX, CATEGORY_PERSONAL, …) unless the workspace has
  # opted in via Settings → Inbox → "Show system labels". User-created and local
  # tags are always visible.
  scope :excluding_system_labels, -> { where(system_label: false) }

  def self.visible_for(workspace)
    workspace&.setting("show_system_labels") ? all : excluding_system_labels
  end

  validates :name, presence: true
  validates :color, presence: true
  validates :name, uniqueness: { scope: :email_account_id }, if: :external?
  validates :name, uniqueness: { scope: :workspace_id }, unless: :external?

  scope :by_name, -> { order(:name) }
  scope :active, -> { where(source: :local).or(where(source: :external)) } # all are active
  scope :grouped, -> { where.not(group_name: nil) }
  scope :ungrouped, -> { where(group_name: nil) }

  def self.group_names(workspace)
    where(workspace: workspace).grouped.pluck(:group_name).uniq.sort
  end

  private

  def enqueue_tag_embedding
    EmbedTagJob.perform_later(self)
  end
end
