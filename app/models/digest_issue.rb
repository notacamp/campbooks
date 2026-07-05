# frozen_string_literal: true

# One materialized occurrence of a ScheduledDigest. Holds the gathered content
# (sections + items + AI overview), delivery status, and audit trail.
#
# Content shape (all string keys):
#   {
#     "overview"  => "1-2 sentence plain-text period overview",
#     "sections"  => [
#       {
#         "title" => "Money & invoices",          # nil when key is present
#         "key"   => "emails",                    # source_type key or "everything_else"
#         "items" => [
#           { "source_type" => "email", "source_id" => "<uuid>",
#             "title" => "...", "subtitle" => "...", "note" => "...",
#             "timestamp" => "<iso8601>" }
#         ]
#       }
#     ],
#     "meta" => { "counts" => { "emails" => 12 }, "list_mode" => false,
#                 "source_errors" => [] }
#   }
class DigestIssue < ApplicationRecord
  belongs_to :scheduled_digest, inverse_of: :issues
  belongs_to :workspace, optional: true   # denormalized — uuid columns, no FK
  belongs_to :user, optional: true        # denormalized — uuid columns, no FK

  # Prefixed to avoid collisions with other enum keys (Rails enum gotcha).
  enum :status, { pending: 0, generated: 1, empty: 2, failed: 3 }, prefix: true

  validates :period_start, presence: true
  validates :period_end, presence: true

  # Convenience accessors that tolerate missing / partially-written content.
  def overview
    content["overview"].to_s
  end

  def sections
    Array(content["sections"])
  end

  def item_count
    sections.sum { |s| Array(s["items"]).size }
  end

  def list_mode?
    content.dig("meta", "list_mode") == true
  end
end
