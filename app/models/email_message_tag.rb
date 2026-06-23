class EmailMessageTag < ApplicationRecord
  belongs_to :email_message
  belongs_to :tag

  validates :email_message_id, uniqueness: { scope: :tag_id }
end
