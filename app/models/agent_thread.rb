class AgentThread < ApplicationRecord
  belongs_to :workspace
  belongs_to :user
  belongs_to :contextable, polymorphic: true, optional: true
  has_many :agent_messages, dependent: :destroy
  has_many :thread_follows, dependent: :destroy
  has_many :followers, through: :thread_follows, source: :user

  enum :purpose, { global: 0, email_chat: 1, compose_chat: 2, setup_chat: 3 }, default: :global

  scope :recent, -> { order(created_at: :desc) }
  scope :with_messages, -> { joins(:agent_messages).group("agent_threads.id").having("COUNT(agent_messages.id) > 0") }
  # Setup chats are an internal onboarding aid, not part of the user's Scout
  # history — keep them out of the sidebar/thread switcher.
  scope :scout_visible, -> { where.not(purpose: :setup_chat) }

  validates :title, presence: true

  def self.default_for(user)
    thread = user.agent_threads.with_messages.where(purpose: :global).order(updated_at: :desc).first
    return thread if thread

    user.agent_threads.create!(title: "New chat", workspace_id: user.workspace_id)
  end

  def context_label
    case purpose.to_sym
    when :email_chat then "Email thread"
    when :compose_chat then "Compose"
    else nil
    end
  end

  def context_url
    if email_chat? && contextable && contextable.respond_to?(:latest_message)
      Rails.application.routes.url_helpers.email_message_path(contextable.latest_message)
    end
  end
end
