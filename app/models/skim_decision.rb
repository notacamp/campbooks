# frozen_string_literal: true

# One triage decision a user made in Skim — "I archived / kept / pinned a cluster
# like this". There is no rules table or training data: these records ARE the
# memory. Emails::SkimActionMemory tallies them per sender / domain / category and,
# once a clear majority emerges, Skim pre-suggests the same action (surfaced as
# Scout) on the next similar card. The email twin of Documents::ClassificationMemory.
#
# We log ONE decision per cluster-decision — archiving a 47-email stack is a single
# "archive", not 47 — so consensus reflects cards acted on, not raw email volume.
# Only the recurring per-card verbs are recorded (keep / archive / promote); the
# sender-state toggles (star / block / allow) are one-off and would make no sense to
# re-suggest.
class SkimDecision < ApplicationRecord
  belongs_to :user
  belongs_to :workspace
  belongs_to :contact, optional: true

  ACTIONS = %w[keep archive promote].freeze

  validates :action, inclusion: { in: ACTIONS }
end
