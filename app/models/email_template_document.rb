# frozen_string_literal: true

# Join between an EmailTemplate and a DocumentTemplate it attaches: when the
# email template is applied, each attached document template is filled with the
# same variables, rendered to a PDF, and attached to the outgoing mail.
class EmailTemplateDocument < ApplicationRecord
  belongs_to :email_template
  belongs_to :document_template

  validates :document_template_id, uniqueness: { scope: :email_template_id }
end
