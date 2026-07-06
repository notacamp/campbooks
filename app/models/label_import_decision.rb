# frozen_string_literal: true

# Tracks the user's review outcome for each provider label that appears during
# label sync. Once a decision row exists for a (account, label_id) pair, the
# label-review banner leaves it alone — even if the same label reappears on the
# next sync. Decisions are additive; they never delete anything.
#
# Decision states:
#   pending  — label discovered, not yet reviewed by the user
#   mapped   — user linked this label to an existing workspace tag
#   kept     — user kept the auto-generated external tag as its own workspace tag,
#              or this was an existing external tag when the feature was introduced
#   ignored  — user decided no workspace tag is needed for this provider label
class LabelImportDecision < ApplicationRecord
  belongs_to :email_account
  belongs_to :tag, optional: true
  belongs_to :reviewed_by, class_name: "User", optional: true

  enum :decision, { pending: 0, mapped: 1, kept: 2, ignored: 3 }, prefix: :decision

  validates :provider_label_id,   presence: true
  validates :provider_label_name, presence: true
  validates :provider_label_id, uniqueness: {
    scope: :email_account_id,
    message: "already has a decision for this account"
  }

  # Helper scopes used by the review controller and the sync services.
  scope :pending_review, -> { where(decision: :pending) }
  scope :resolved,       -> { where.not(decision: :pending) }
  scope :for_workspace,  ->(workspace) {
    joins(:email_account).where(email_accounts: { workspace_id: workspace.id })
  }

  # Mark this decision as resolved and stamp the reviewer + time. Idempotent:
  # calling it on an already-resolved row is a no-op.
  def resolve!(decision:, tag: nil, reviewed_by: nil)
    return if decision_pending? == false

    update!(
      decision: decision,
      tag: tag,
      reviewed_by: reviewed_by,
      reviewed_at: Time.current
    )
  end
end
