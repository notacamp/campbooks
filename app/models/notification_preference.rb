class NotificationPreference < ApplicationRecord
  belongs_to :user
  belongs_to :tag, optional: true
  belongs_to :document_type, optional: true

  enum :kind, { tag: 0, document_type: 1 }

  validates :kind, presence: true
  validates :tag_id, uniqueness: { scope: [ :user_id, :kind ] }, if: :tag?
  validates :document_type_id, uniqueness: { scope: [ :user_id, :kind ] }, if: :document_type?
end
