# frozen_string_literal: true

# A reusable email (subject + HTML body) carrying Liquid {{ variables }}, the
# email-content sibling of DocumentTemplate. Created manually or AI-generated
# (EmailTemplates::HtmlGenerator), used from the composer via
# EmailTemplates::Applier, and optionally schedulable. May carry attached
# DocumentTemplates that render to PDF attachments when the template is applied.
class EmailTemplate < ApplicationRecord
  belongs_to :workspace
  has_many :email_template_documents, -> { order(:position) }, dependent: :destroy
  has_many :document_templates, through: :email_template_documents

  enum :ai_status, { pending: 0, processing: 1, completed: 2, failed: 3 }, prefix: :ai

  validates :name, presence: true, length: { maximum: 255 }
  validates :subject, length: { maximum: 998 } # RFC 5322 line limit
  validates :body_html, length: { maximum: 100_000 }

  scope :recent, -> { order(created_at: :desc) }
  # Templates worth showing in the composer picker — those with a body to insert.
  scope :usable, -> { where.not(body_html: [ nil, "" ]) }

  # The AI-generated variable definitions ([{key, label, type, required, default}]).
  def variable_definitions = variables_schema.presence || []

  # Variable names actually referenced anywhere in the subject or body.
  def extract_used_variables
    "#{subject} #{body_html}".scan(/\{\{\s*(\w+)\s*\}\}/).flatten.uniq
  end

  def rendered_subject(variables = {}) = render_liquid(subject, variables)
  def rendered_body(variables = {}) = render_liquid(body_html, variables)

  private

  def render_liquid(template, variables)
    return "" if template.blank?

    Liquid::Template.parse(template, error_mode: :strict)
                    .render!((variables || {}).deep_stringify_keys, strict_variables: false, strict_filters: true)
  rescue Liquid::Error
    template.to_s
  end
end
