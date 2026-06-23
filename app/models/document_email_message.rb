class DocumentEmailMessage < ApplicationRecord
  belongs_to :document
  belongs_to :email_message

  validates :document_id, uniqueness: { scope: :email_message_id }
end
