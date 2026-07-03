# One recorded human decision on an AI suggestion that had no durable verdict
# record of its own (a Skim cluster the user triaged, a feed tag-suggestion they
# acted on or dismissed). These rows ARE the memory: Learning::Sources::Decisions
# tallies them per signal (contact / domain / category / …) so the matching loop
# can pre-suggest or suppress the same choice next time.
#
# Domains that DO carry their own verdict (documents.review_status,
# reminders.status, tasks.status) are read straight off their own table via a
# dedicated source and never write here — no double-write.
class LearningDecision < ApplicationRecord
  belongs_to :workspace
  belongs_to :user
  belongs_to :contact, optional: true
  belongs_to :subject, polymorphic: true, optional: true

  validates :domain, presence: true
  validates :label, presence: true
end
