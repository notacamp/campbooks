# frozen_string_literal: true

module Documents
  # Moves Document's extracted scalar values from typed columns to the
  # +metadata+ JSONB column, accessed through typed reader/writer pairs.
  #
  # The macro +extracted_field :name, :type+ defines:
  #   - A reader that decodes the value from metadata with the right Ruby type.
  #   - A writer that coerces and stores the value in metadata (never in the
  #     legacy column), deleting the key when the coerced value is nil.
  #   - A predicate +name?+ for :boolean fields.
  #
  # All 23 extracted fields are declared at the bottom of the included block.
  # Money readers (amount, tax_amount, opening_balance, closing_balance) wrap
  # the cents value in a Money object using the document's +currency+.
  module ExtractedFields
    extend ActiveSupport::Concern

    # Columns that previously held extracted values — now shadowed by metadata.
    # ActiveRecord ignores these so column-level getters/setters are never
    # generated; the macro writers below own the attr contract instead.
    IGNORED_COLUMN_NAMES = %w[
      vendor_name vendor_nif client_name client_nif buyer_nif bank_name
      sender_name account_number invoice_number receipt_number payment_method
      amount_cents tax_amount_cents tax_rate opening_balance_cents
      closing_balance_cents currency document_date due_date period_start
      period_end expense_category company_vat_present
    ].freeze

    included do # rubocop:disable Metrics/BlockLength
      self.ignored_columns += IGNORED_COLUMN_NAMES

      # ── String fields ─────────────────────────────────────────────────────
      extracted_field :vendor_name,     :string
      extracted_field :vendor_nif,      :string
      extracted_field :client_name,     :string
      extracted_field :client_nif,      :string
      extracted_field :buyer_nif,       :string
      extracted_field :bank_name,       :string
      extracted_field :sender_name,     :string
      extracted_field :account_number,  :string
      extracted_field :invoice_number,  :string
      extracted_field :receipt_number,  :string
      extracted_field :payment_method,  :string

      # ── Integer (cents) fields ────────────────────────────────────────────
      extracted_field :amount_cents,            :integer
      extracted_field :tax_amount_cents,        :integer
      extracted_field :opening_balance_cents,   :integer
      extracted_field :closing_balance_cents,   :integer

      # ── Number field ──────────────────────────────────────────────────────
      extracted_field :tax_rate, :number

      # ── Date fields ───────────────────────────────────────────────────────
      extracted_field :document_date, :date
      extracted_field :due_date,      :date
      extracted_field :period_start,  :date
      extracted_field :period_end,    :date

      # ── Boolean field ─────────────────────────────────────────────────────
      extracted_field :company_vat_present, :boolean

      # ── Currency (string, defaults to "EUR") ─────────────────────────────
      extracted_field :currency, :string

      # Override the generated reader to default to "EUR" when blank.
      define_method(:currency) do
        raw = metadata&.dig("currency")
        raw.presence || "EUR"
      end

      # ── Expense category (enum-ish with legacy int mapping) ───────────────
      # Declared as a plain string extracted_field first, then the writer is
      # replaced to handle legacy integer assignments (old enum order: travel=0…
      # other=9) and ensure values are validated against EXPENSE_CATEGORIES.
      extracted_field :expense_category, :string

      define_method(:expense_category=) do |value|
        coerced = if value.is_a?(Integer) || (value.is_a?(String) && value.match?(/\A\d+\z/))
                    Document::EXPENSE_CATEGORIES[value.to_i]
        elsif value.present?
                    str = value.to_s
                    Document::EXPENSE_CATEGORIES.include?(str) ? str : nil
        end

        if coerced.nil?
          self.metadata = (metadata || {}).except("expense_category")
        else
          self.metadata = (metadata || {}).merge("expense_category" => coerced)
        end
      end

      # ── Money readers ─────────────────────────────────────────────────────
      # Wrap cents in a Money object using the document's currency. Returns nil
      # when the cents field is nil. Fixes the old monetize hardcoded-EUR bug —
      # the currency is now read from metadata (defaulting to EUR) so a document
      # stored with "USD" in metadata will return a USD Money object.
      define_method(:amount) do
        cents = amount_cents
        Money.new(cents, currency) unless cents.nil?
      end

      define_method(:tax_amount) do
        cents = tax_amount_cents
        Money.new(cents, currency) unless cents.nil?
      end

      define_method(:opening_balance) do
        cents = opening_balance_cents
        Money.new(cents, currency) unless cents.nil?
      end

      define_method(:closing_balance) do
        cents = closing_balance_cents
        Money.new(cents, currency) unless cents.nil?
      end
    end

    # ── Class methods ─────────────────────────────────────────────────────────

    class_methods do
      # Define a reader and writer for +name+ backed by the metadata JSONB column.
      def extracted_field(name, type)
        @extracted_field_names ||= []
        @extracted_field_names << name.to_s

        # Reader — typed decode from metadata.
        define_method(name) do
          DocumentTypes::Coercion.read(type, metadata, name.to_s)
        end

        # Writer — coerce then store in metadata, or delete the key when nil.
        define_method(:"#{name}=") do |value|
          coerced = DocumentTypes::Coercion.coerce(type, value)

          self.metadata = if coerced.nil?
                            (metadata || {}).except(name.to_s)
          else
                            (metadata || {}).merge(name.to_s => coerced)
          end
        end

        # Predicate for boolean fields.
        if type == :boolean
          define_method(:"#{name}?") { !!public_send(name) }
        end
      end

      # Array of extracted field name strings in declaration order.
      def extracted_field_names
        @extracted_field_names&.dup || []
      end
    end

    # ── Instance methods ──────────────────────────────────────────────────────

    # Returns true when +key+'s value differs between the before and after
    # snapshots of the metadata column in the most recent save.
    # Nil-safe: missing metadata before or after is treated as {}.
    def saved_change_to_extracted_field?(key)
      return false unless saved_changes.key?("metadata")

      before, after = saved_changes["metadata"]
      (before || {}).fetch(key.to_s, nil) != (after || {}).fetch(key.to_s, nil)
    end
  end
end
