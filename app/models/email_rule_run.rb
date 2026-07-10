# frozen_string_literal: true

# Records a retroactive sweep of the inbox against a single EmailRule.
# Tracks progress (matched_count / processed_count) and stores enough
# information to undo the run when undoable is true.
#
# Undo data (archived_email_ids / marked_read_email_ids / moved_email_ids)
# is only written when the match set is <= 25_000 (undoable: true); above
# that threshold the arrays stay empty and the run is not undoable.
class EmailRuleRun < ApplicationRecord
  belongs_to :email_rule
  belongs_to :workspace
  belongs_to :started_by, class_name: "User", optional: true

  enum :status, {
    queued:    0,
    running:   1,
    completed: 2,
    undone:    3,
    failed:    4
  }

  validates :email_rule, presence: true
  validates :workspace,  presence: true
end
