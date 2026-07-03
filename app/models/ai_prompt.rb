# Per-workspace custom guidance for one AI purpose (see Ai::PromptCatalog).
#
# The text is APPENDED to the built-in prompt via
# Ai::Configuration.user_prompt_suffix — it never replaces it, so the model's
# output schema and safety rules stay authoritative. A row exists only when a
# workspace has actually written custom instructions; clearing them deletes the
# row and restores the default behavior.
#
# Deliberately separate from AiConfiguration (which pins model/adapter routing):
# a user can add guidance for a feature without first assigning it a model.
class AiPrompt < ApplicationRecord
  belongs_to :workspace

  MAX_LENGTH = 2000

  validates :purpose,
    presence: true,
    inclusion: { in: Ai::PromptCatalog::KEYS },
    uniqueness: { scope: :workspace_id }
  validates :instructions, length: { maximum: MAX_LENGTH }, allow_blank: true

  # Rows with meaningful, non-blank guidance.
  scope :configured, -> { where.not(instructions: [ nil, "" ]) }

  def catalog_entry
    Ai::PromptCatalog.find(purpose)
  end

  def configured?
    instructions.present?
  end
end
