# frozen_string_literal: true

# Pointer from a workspace tag to a provider label in a specific email account.
# One row per (tag, account) pair — a tag can be linked to the same label name
# across multiple accounts, but each account has at most one link per tag and one
# tag per label.
#
# Coexists with the legacy `email_account_id` / `external_label_id` columns on
# `tags` — those columns are left intact for backward compatibility with existing
# sync code.
class TagAccountLink < ApplicationRecord
  belongs_to :tag
  belongs_to :email_account

  validates :provider_label_id, presence: true
  validates :tag_id, uniqueness: {
    scope: :email_account_id,
    message: "already linked to this account"
  }
  validates :provider_label_id, uniqueness: {
    scope: :email_account_id,
    message: "already linked to a tag in this account"
  }

  # Workspace safety check — both the tag and the account must belong to the same
  # workspace. Enforced at the model layer to guard the merge service and any
  # bulk operations.
  validate :same_workspace

  private

  def same_workspace
    return unless tag && email_account
    return if tag.workspace_id == email_account.workspace_id

    errors.add(:base, "tag and account must belong to the same workspace")
  end
end
