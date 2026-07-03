# frozen_string_literal: true

# DEPRECATED as a write target. New Skim triage decisions are recorded as generic
# LearningDecision rows (domain: "email_skim") and read back by the Learning::
# substrate; the historical rows here were copied over by
# BackfillSkimDecisionsToLearningDecisions. This table is retained read-only until a
# later release drops it once production row-count parity is confirmed. ACTIONS below
# is still the canonical taxonomy of learnable Skim verbs (referenced by the recorder).
#
# Original intent: one triage decision a user made in Skim — "I archived / kept /
# pinned a cluster like this". We log ONE decision per cluster-decision — archiving a
# 47-email stack is a single "archive", not 47 — so consensus reflects cards acted
# on, not raw email volume. Only the recurring per-card verbs are recorded (keep /
# archive / promote); the sender-state toggles (star / block / allow) are one-off and
# would make no sense to re-suggest.
class SkimDecision < ApplicationRecord
  belongs_to :user
  belongs_to :workspace
  belongs_to :contact, optional: true

  ACTIONS = %w[keep archive promote].freeze

  validates :action, inclusion: { in: ACTIONS }
end
