class Person < ApplicationRecord
  RELATIONSHIP_TYPES = %w[self client vendor partner service_provider colleague personal unknown].freeze

  belongs_to :workspace

  has_many :contacts, foreign_key: :person_id, dependent: :nullify
  has_many :suggested_contacts, class_name: "Contact", foreign_key: :suggested_person_id, dependent: :nullify

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

  def all_emails
    contacts.pluck(:email) + contacts.joins(:contact_email_aliases).pluck(:"contact_email_aliases.email")
  end
end
