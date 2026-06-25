# frozen_string_literal: true

module Documents
  # Single source of truth for a document's "extracted data" field set — the same
  # per-type fields the detail page's `documents/forms/<type>` partials render, so
  # the Skim card mirrors the detail page instead of only echoing the raw metadata
  # hash (which left most documents showing "No data extracted" even though their
  # values live in typed columns).
  #
  # Each field is a Hash: { key:, label:, value:, kind:, store: }
  #   value — raw value (typed column first, metadata fallback); blanks are kept so
  #           the card can show the whole field set and pre-fill the inline editor
  #   kind  — :text | :date | :money | :enum_expense_category | :enum_payment_method
  #   store — :column   → inline edits POST as document[key]  (assign_attributes)
  #           :metadata → inline edits POST as document[metadata][key] (merged)
  #
  # Dependency-light + deterministic so it unit-tests in isolation, mirroring
  # Documents::SkimBuilder.
  class ExtractedFieldSet
    # The five built-in types that have a `forms/<type>` partial; their canonical
    # data lives in typed columns. Entry: [column_key, i18n_label_key, kind] — the
    # label key is the one the matching partial already uses (full i18n parity).
    TYPE_FIELDS = {
      "expense_invoice" => [
        [ :vendor_name, "vendor_name", :text ],
        [ :vendor_nif, "vendor_nif", :text ],
        [ :buyer_nif, "buyer_nif", :text ],
        [ :document_date, "document_date", :date ],
        [ :due_date, "due_date", :date ],
        [ :invoice_number, "invoice_number", :text ],
        [ :amount_cents, "amount_cents", :money ],
        [ :currency, "currency", :text ],
        [ :tax_amount_cents, "tax_amount_cents", :money ],
        [ :tax_rate, "tax_rate", :text ],
        [ :expense_category, "expense_category", :enum_expense_category ]
      ],
      "revenue_invoice" => [
        [ :client_name, "client_name", :text ],
        [ :client_nif, "client_nif", :text ],
        [ :document_date, "document_date", :date ],
        [ :due_date, "due_date", :date ],
        [ :invoice_number, "invoice_number", :text ],
        [ :amount_cents, "amount_cents", :money ],
        [ :currency, "currency", :text ],
        [ :tax_amount_cents, "tax_amount_cents", :money ],
        [ :tax_rate, "tax_rate", :text ]
      ],
      "receipt" => [
        [ :vendor_name, "vendor_name", :text ],
        [ :vendor_nif, "vendor_nif", :text ],
        [ :document_date, "document_date", :date ],
        [ :receipt_number, "receipt_number", :text ],
        [ :amount_cents, "amount_cents", :money ],
        [ :payment_method, "payment_method", :enum_payment_method ]
      ],
      "bank_statement" => [
        [ :bank_name, "bank_name", :text ],
        [ :account_number, "account_number", :text ],
        [ :period_start, "period_start", :date ],
        [ :period_end, "period_end", :date ],
        [ :opening_balance_cents, "opening_balance", :money ],
        [ :closing_balance_cents, "closing_balance", :money ],
        [ :currency, "currency", :text ]
      ],
      "other" => [
        [ :vendor_name, "entity_name", :text ],
        [ :document_date, "document_date", :date ]
      ]
    }.freeze

    # Editable typed columns the inline Skim editor may write — the strong-params
    # allowlist for Documents::SkimController#update_fields.
    COLUMN_KEYS = TYPE_FIELDS.values.flatten(1).map { |key, _label, _kind| key }.uniq.freeze

    # The enum-backed columns among them. The editor offers a blank "unset" option,
    # which the controller coerces to nil before assigning (an enum can't take "").
    ENUM_KEYS = TYPE_FIELDS.values.flatten(1)
                  .select { |_key, _label, kind| kind.to_s.start_with?("enum") }
                  .map { |key, _label, _kind| key }.uniq.freeze

    def initialize(document)
      @document = document
    end

    def fields
      schema = @document.classification&.extraction_schema
      if schema.is_a?(Hash) && schema.any?
        never_blank(schema_fields(schema))
      elsif (defn = TYPE_FIELDS[@document.document_type.to_s])
        never_blank(column_fields(defn))
      else
        metadata_fields
      end
    end

    private

    # Never return an all-empty field set when the AI actually extracted data into
    # metadata under keys outside the typed/schema set — e.g. a boarding pass filed
    # as "other", whose flight_number/gate/seat live in metadata but not in the
    # type's two columns. Fall back to the raw metadata so nothing extracted is hidden.
    def never_blank(fields)
      return fields if fields.any? { |f| f[:value].to_s.strip.present? }

      metadata_fields.presence || fields
    end

    def metadata
      @metadata ||= (@document.metadata.presence || {})
    end

    # Custom type with a JSON extraction_schema: canonical labels + order from the
    # schema, values + inline edits in metadata.
    def schema_fields(schema)
      schema.map do |name, defn|
        defn = {} unless defn.is_a?(Hash)
        label = defn["description"].presence || name.to_s.humanize
        { key: name.to_s, label: label, value: metadata[name.to_s], kind: :text, store: :metadata }
      end
    end

    # Built-in type: the partial's field set, sourced from the typed column (falling
    # back to metadata for legacy rows that only populated the column), edited as a
    # column — exactly what the detail page does.
    def column_fields(defn)
      type = @document.document_type.to_s
      defn.map do |key, label_key, kind|
        column = @document.respond_to?(key) ? @document.public_send(key) : nil
        value = column.presence || metadata[key.to_s]
        { key: key.to_s, label: I18n.t("documents.forms.#{type}.#{label_key}"),
          value: value, kind: kind, store: :column }
      end
    end

    # Unknown/custom type without a Hash schema (e.g. a free-form type): surface
    # whatever the AI wrote into metadata so nothing extracted is ever hidden.
    def metadata_fields
      metadata.except("title").map do |name, value|
        { key: name.to_s, label: name.to_s.humanize, value: value, kind: :text, store: :metadata }
      end
    end
  end
end
