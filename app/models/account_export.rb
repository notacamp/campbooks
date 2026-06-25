# A user-requested, asynchronously-built archive of their own personal data — the
# GDPR right-to-portability copy. AccountExportJob zips the DataExporter JSON plus
# the user's email bodies, email attachments, and document files into `archive`.
# Distinct from the workspace-scoped Export (which packages document PDFs).
class AccountExport < ApplicationRecord
  belongs_to :user

  has_one_attached :archive

  enum :status, {
    pending: 0,
    generating: 1,
    generated: 2,
    failed: 3
  }

  validates :status, presence: true

  scope :recent, -> { order(created_at: :desc) }
end
