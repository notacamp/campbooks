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

  # Why a tag is/isn't shown as a chip — decided once per provider label and then
  # remembered (see classified_at). Provider system statuses (INBOX, read, …) and
  # AI-judged low-value labels (e.g. Gmail's CATEGORY_UPDATES) are hidden; genuine
  # user labels stay visible. User-created/local tags are always :user.
  enum :kind, { user: 0, system: 1, category: 2, low_value: 3 }, default: :user, prefix: :kind

  # The single visibility gate used at every render site. `hidden` is the source
  # of truth (a provider system status, or an AI/user decision); it is overridable
  # per-label in Settings → Tags.
  scope :visible, -> { where(hidden: false) }
  scope :hidden_labels, -> { where(hidden: true) }

  # Back-compat: still keyed on the legacy system_label flag. Prefer `visible`.
  scope :excluding_system_labels, -> { where(system_label: false) }

  # The visibility gate used at every render site. Hidden tags (provider system
  # statuses + AI-judged low-value labels) are filtered out, unless the workspace
  # opts to see everything via the legacy "show system labels" setting. (That
  # toggle is superseded by the per-label review in Settings → Tags.)
  def self.visible_for(workspace)
    workspace&.setting("show_system_labels") ? all : visible
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

  # Persist a classification decision (see Labels::Classifier / AiClassifier)
  # without firing the embedding callback — classification metadata isn't part of
  # the embedding content, so update_columns is the right tool here.
  def apply_classification!(kind:, hidden:, confidence: nil, reason: nil)
    update_columns(
      kind: self.class.kinds.fetch(kind.to_s),
      hidden: hidden,
      classified_at: Time.current,
      classification_confidence: confidence,
      classification_reason: reason
    )
  end

  private

  def enqueue_tag_embedding
    EmbedTagJob.perform_later(self)
  end
end
