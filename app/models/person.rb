class Person < ApplicationRecord
  RELATIONSHIP_TYPES = %w[self client vendor partner service_provider colleague personal unknown].freeze

  belongs_to :workspace

  has_many :contacts, foreign_key: :person_id, dependent: :nullify
  has_many :suggested_contacts, class_name: "Contact", foreign_key: :suggested_person_id, dependent: :nullify

  has_many :organization_memberships, dependent: :destroy
  has_many :organizations, through: :organization_memberships
  has_many :active_organization_memberships, -> { active }, class_name: "OrganizationMembership"
  has_many :active_organizations, through: :active_organization_memberships, source: :organization
  has_one :primary_organization_membership, -> { active.order(created_at: :desc) }, class_name: "OrganizationMembership"
  has_one :primary_organization, through: :primary_organization_membership, source: :organization

  def display_name
    name.presence || contacts.first&.display_name || "Unknown"
  end

  def analyzed?
    analyzed_at.present?
  end

  def needs_analysis?
    analyzed_at.nil? || analyzed_at < 30.days.ago
  end

  def total_email_count
    if contacts.loaded?
      contacts.sum { |c| c.email_count || 0 }
    else
      contacts.sum(:email_count)
    end
  end

  def last_email_at
    if contacts.loaded?
      contacts.filter_map(&:last_email_at).max
    else
      contacts.maximum(:last_email_at)
    end
  end

  def primary_email
    contacts.order(email_count: :desc).first&.email
  end

  def organization_name
    primary_organization&.name || read_attribute(:organization)
  end

  def all_emails
    contacts.pluck(:email) + contacts.joins(:contact_email_aliases).pluck(:"contact_email_aliases.email")
  end
end
