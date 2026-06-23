class ContactEmailAlias < ApplicationRecord
  belongs_to :contact

  validates :email, presence: true, uniqueness: true
end
