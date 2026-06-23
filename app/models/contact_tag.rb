class ContactTag < ApplicationRecord
  belongs_to :contact
  belongs_to :tag

  # auto = assigned by sender analysis; manual = set by a person.
  enum :source, { auto: 0, manual: 1 }

  validates :tag_id, uniqueness: { scope: :contact_id }
end
