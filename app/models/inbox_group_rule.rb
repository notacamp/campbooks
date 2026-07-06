# frozen_string_literal: true

# A single rule that adds threads to an inbox group by matching a property of
# the message rather than a tag. A group's full membership is the union of its
# tagged threads and all its rule-based threads — "additive multi-membership"
# means the same thread can satisfy multiple groups simultaneously.
#
# Rule types
#   sender        value = bare email address ("bob@example.com") or "@domain"
#                 ("@example.com"). Matched case-insensitively against
#                 email_messages.from_address with ILIKE.
#   organization  value = Organization uuid. Matches messages whose contact
#                 belongs (via person/organization_membership) to that org.
#   document_type value = DocumentType uuid. Matches messages linked to a
#                 document of that type via document_email_messages.
#   query         value = a structured-filter search string understood by
#                 Emails::SearchQuery (from:, to:, subject:, has:, is:,
#                 after:, before:, tag:, category:, priority:). Free-text
#                 terms are silently ignored; only modifier filters apply.
class InboxGroupRule < ApplicationRecord
  RULE_TYPES = %w[sender organization document_type query].freeze

  belongs_to :workspace

  validates :group_name, presence: true
  validates :rule_type, presence: true, inclusion: { in: RULE_TYPES }
  validates :value, presence: true

  scope :for_group, ->(name) { where(group_name: name.to_s) }
  scope :ordered, -> { order(:rule_type, :value) }
end
