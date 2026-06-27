class DocumentTemplate < ApplicationRecord
  belongs_to :workspace
  has_one_attached :preview_pdf

  enum :ai_status, { pending: 0, processing: 1, completed: 2, failed: 3 }, prefix: :ai

  validates :name, presence: true, length: { maximum: 255 }
  validates :html_content, length: { maximum: 100_000 }

  scope :recent, -> { order(created_at: :desc) }

  # The AI-produced variable schema: an array of
  # { "key", "label", "type", "required", "default" } hashes. Always an array.
  def variable_definitions
    variables_schema.presence || []
  end

  # The Liquid variable names actually referenced in the body (`{{ name }}`), so
  # the fill form only pre-fills fields the template really uses.
  def extract_used_variables
    return [] if html_content.blank?

    html_content.scan(/\{\{\s*(\w+)\s*\}\}/).flatten.uniq
  end

  # Render the template with the given variables. Delegates to the shared Liquid
  # renderer so the model and the Sender service stay in lockstep.
  def rendered_html(variables = {})
    DocumentTemplates::Filler.call(html_content, variables)
  end
end
