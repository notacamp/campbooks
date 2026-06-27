class DocumentTemplate < ApplicationRecord
  belongs_to :workspace
  has_one_attached :preview_pdf
  enum :ai_status, { pending: 0, processing: 1, completed: 2, failed: 3 }, prefix: :ai
  validates :name, presence: true, length: { maximum: 255 }
  validates :html_content, length: { maximum: 100_000 }
  scope :recent, -> { order(created_at: :desc) }
  def variable_definitions = variables_schema.presence || []
  def extract_used_variables
    return [] if html_content.blank?
    html_content.scan(/\{\{\s*(\w+)\s*\}\}/).flatten.uniq
  end
  def rendered_html(variables = {})
    return "" if html_content.blank?
    Liquid::Template.parse(html_content, error_mode: :strict)
      .render!(variables.deep_stringify_keys, strict_variables: false, strict_filters: true)
  rescue Liquid::Error
    ""
  end
end
