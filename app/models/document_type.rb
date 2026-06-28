class DocumentType < ApplicationRecord
  CATEGORIES = %w[accounting legal insurance vehicles identification correspondence other].freeze

  # Best-guess category for a type NAME, so every provisioning path (setup wizard, AI
  # analyzer, onboarding, the AI assistant) ends up with a categorised type — they used
  # to create category-less types, which then vanished from the reclassify picker and
  # all piled into the "Other" review ring. Keyed by the built-in document_type enum
  # values plus the setup presets' names; everything else falls back to "other".
  DEFAULT_CATEGORIES = {
    "expense_invoice"    => "accounting",
    "revenue_invoice"    => "accounting",
    "credit_note"        => "accounting",
    "bank_statement"     => "accounting",
    "bank_journal_entry" => "accounting",
    "receipt"            => "accounting",
    "tax_document"       => "accounting",
    "invoice"            => "accounting",
    "payslip"            => "accounting",
    "contract"           => "legal",
    "proposal"           => "legal",
    "insurance_policy"   => "insurance",
    "vehicle_document"   => "vehicles",
    "certificate"        => "identification",
    "identification"     => "identification",
    "correspondence"     => "correspondence"
  }.freeze

  # "Bank Statement" / "bank statement" / "bank_statement" all resolve the same.
  def self.default_category_for(name)
    DEFAULT_CATEGORIES.fetch(name.to_s.strip.downcase.tr(" ", "_"), "other")
  end

  has_rich_text :prompt

  # Return plain text (markdown) from the rich text body instead of HTML.
  def prompt
    rich_text_prompt&.body&.to_plain_text.presence
  end

  # ActionText's generated `prompt=` reads back through the overridden getter
  # above (which returns a String, not the RichText record), so it blows up with
  # "undefined method `body='". Write to the association directly instead.
  def prompt=(value)
    (rich_text_prompt || build_rich_text_prompt).body = value
  end

  belongs_to :workspace

  has_many :documents, dependent: :nullify
  has_many :notification_preferences, dependent: :destroy
  has_one :notion_database_mapping, dependent: :destroy
  has_one :google_drive_config, dependent: :destroy
  accepts_nested_attributes_for :notion_database_mapping, allow_destroy: true

  validates :name, presence: true, uniqueness: { scope: :workspace_id }
  validates :color, presence: true
  validates :category, inclusion: { in: CATEGORIES }, allow_nil: true

  # New types get a sensible category unless one was set explicitly. Scoped to :create
  # so a user can still clear the category on an existing type via the edit form.
  before_validation :assign_default_category, on: :create

  scope :by_category, ->(cat) { where(category: cat) if cat.present? }

  private

  def assign_default_category
    self.category = self.class.default_category_for(name) if category.blank?
  end
end
