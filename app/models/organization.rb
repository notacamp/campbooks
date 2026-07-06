class Organization < ApplicationRecord
  belongs_to :workspace
  has_many :organization_memberships, dependent: :destroy
  has_many :people, through: :organization_memberships
  has_many :active_memberships, -> { active }, class_name: "OrganizationMembership"
  has_many :active_people, through: :active_memberships, source: :person
  has_many :contacts, -> { distinct }, through: :people
  has_many :email_messages, -> { distinct }, through: :contacts
  has_many :documents, -> { distinct }, through: :email_messages

  validates :name, presence: true
  validates :name, uniqueness: { scope: :workspace_id, message: :taken }

  scope :ordered, -> { order(:name) }
  scope :by_name, ->(name) { where("name ILIKE ?", "%#{sanitize_sql_like(name)}%") }
  # Directory search: match the org name or its email domain (case-insensitive).
  scope :search, ->(term) {
    like = "%#{sanitize_sql_like(term.to_s.strip)}%"
    where("name ILIKE :like OR domain ILIKE :like", like: like)
  }

  def member_count = people.size
  def active_member_count = active_people.count
  def email_count = email_messages.count
end
