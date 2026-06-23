class AgentMessage < ApplicationRecord
  belongs_to :user
  belongs_to :agent_thread, optional: true, touch: true
  has_many :linked_email_messages, class_name: "EmailMessage", foreign_key: :ai_analysis_message_id, dependent: :nullify

  enum :author_type, { user: 0, ai: 1 }
  enum :reply_status, { pending: 0, replied: 1, failed: 2, processing: 3 }

  scope :chronological, -> { order(created_at: :asc) }
  scope :needing_reply, -> { where(author_type: :user, reply_status: :pending).order(created_at: :asc) }

  validates :content, presence: true

  # Tagging @scout (case-insensitive, not mid-word so foo@scout.com is ignored)
  # is what invokes the AI in an email discussion thread. Plain comments between
  # teammates stay human-to-human.
  SCOUT_MENTION = /(?<!\w)@scout\b/i

  before_destroy :clear_linked_analysis

  def author_name
    ai? ? "Scout" : user&.name || "User"
  end

  def from_user?
    user?
  end

  def from_ai?
    ai?
  end

  def mentions_scout?
    content.to_s.match?(SCOUT_MENTION)
  end

  private

  def clear_linked_analysis
    linked_email_messages.update_all(
      ai_summary: nil,
      ai_action_prompt: nil,
      ai_suggested_actions: [],
      ai_analysis_message_id: nil
    )
  end
end
