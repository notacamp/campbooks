class NotionIntegration < ApplicationRecord
  belongs_to :workspace
  belongs_to :authorized_by_user, class_name: "User", optional: true

  encrypts :access_token

  validates :access_token, presence: true

  scope :active, -> { where(active: true) }

  def deactivate!
    update!(active: false)
  end

  # Display label for a connected workspace (OAuth populates the name; manual-token
  # integrations may not have one).
  def display_name
    notion_workspace_name.presence || "Notion"
  end

  # True for OAuth-connected integrations (vs a manually-pasted internal token).
  def oauth?
    bot_id.present? || notion_workspace_id.present?
  end
end
