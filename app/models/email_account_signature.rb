class EmailAccountSignature < ApplicationRecord
  belongs_to :signature
  belongs_to :email_account

  validates :signature_id, uniqueness: { scope: :email_account_id }
end
