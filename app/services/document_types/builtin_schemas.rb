# frozen_string_literal: true

module DocumentTypes
  # Canonical enriched extraction schemas for the five built-in document types.
  #
  # Each entry is a field-name → schema-hash mapping identical in shape to what
  # a DocumentType#extraction_schema JSONB column stores, enriched with:
  #   "type"        — money / date / number / enum / string
  #   "label_key"   — full I18n key that the matching form partial already uses
  #   "description" — English label (mirrors the en locale string)
  #   "position"    — 1-based display order
  #   "values"      — (enum only) allowed string values
  #
  # Usage:
  #   DocumentTypes::BuiltinSchemas.for("expense_invoice")   # → Hash or nil
  #   DocumentTypes::BuiltinSchemas::ALL                      # → { name => schema }
  module BuiltinSchemas
    EXPENSE_CATEGORIES = %w[
      travel meals office_supplies utilities rent software
      professional_services equipment marketing other
    ].freeze

    PAYMENT_METHODS = %w[cash card transfer mbway multibanco check other].freeze

    ALL = {
      "expense_invoice" => {
        "vendor_name"     => { "type" => "string", "position" => 1,
                               "label_key"   => "documents.forms.expense_invoice.vendor_name",
                               "description" => "Vendor Name" },
        "vendor_nif"      => { "type" => "string", "position" => 2,
                               "label_key"   => "documents.forms.expense_invoice.vendor_nif",
                               "description" => "Vendor NIF" },
        "buyer_nif"       => { "type" => "string", "position" => 3,
                               "label_key"   => "documents.forms.expense_invoice.buyer_nif",
                               "description" => "Buyer NIF" },
        "document_date"   => { "type" => "date",   "position" => 4,
                               "label_key"   => "documents.forms.expense_invoice.document_date",
                               "description" => "Document Date" },
        "due_date"        => { "type" => "date",   "position" => 5,
                               "label_key"   => "documents.forms.expense_invoice.due_date",
                               "description" => "Due Date" },
        "invoice_number"  => { "type" => "string", "position" => 6,
                               "label_key"   => "documents.forms.expense_invoice.invoice_number",
                               "description" => "Invoice Number" },
        "amount_cents"    => { "type" => "money",  "position" => 7,
                               "label_key"   => "documents.forms.expense_invoice.amount_cents",
                               "description" => "Amount (cents)" },
        "currency"        => { "type" => "string", "position" => 8,
                               "label_key"   => "documents.forms.expense_invoice.currency",
                               "description" => "Currency" },
        "tax_amount_cents" => { "type" => "money", "position" => 9,
                                "label_key"   => "documents.forms.expense_invoice.tax_amount_cents",
                                "description" => "Tax Amount (cents)" },
        "tax_rate"        => { "type" => "number", "position" => 10,
                               "label_key"   => "documents.forms.expense_invoice.tax_rate",
                               "description" => "Tax Rate (%)" },
        "expense_category" => { "type" => "enum",  "position" => 11,
                                "values"      => EXPENSE_CATEGORIES,
                                "label_key"   => "documents.forms.expense_invoice.expense_category",
                                "description" => "Expense Category" }
      }.freeze,

      "revenue_invoice" => {
        "client_name"     => { "type" => "string", "position" => 1,
                               "label_key"   => "documents.forms.revenue_invoice.client_name",
                               "description" => "Client Name" },
        "client_nif"      => { "type" => "string", "position" => 2,
                               "label_key"   => "documents.forms.revenue_invoice.client_nif",
                               "description" => "Client NIF" },
        "document_date"   => { "type" => "date",   "position" => 3,
                               "label_key"   => "documents.forms.revenue_invoice.document_date",
                               "description" => "Document Date" },
        "due_date"        => { "type" => "date",   "position" => 4,
                               "label_key"   => "documents.forms.revenue_invoice.due_date",
                               "description" => "Due Date" },
        "invoice_number"  => { "type" => "string", "position" => 5,
                               "label_key"   => "documents.forms.revenue_invoice.invoice_number",
                               "description" => "Invoice Number" },
        "amount_cents"    => { "type" => "money",  "position" => 6,
                               "label_key"   => "documents.forms.revenue_invoice.amount_cents",
                               "description" => "Amount (cents)" },
        "currency"        => { "type" => "string", "position" => 7,
                               "label_key"   => "documents.forms.revenue_invoice.currency",
                               "description" => "Currency" },
        "tax_amount_cents" => { "type" => "money", "position" => 8,
                                "label_key"   => "documents.forms.revenue_invoice.tax_amount_cents",
                                "description" => "Tax Amount (cents)" },
        "tax_rate"        => { "type" => "number", "position" => 9,
                               "label_key"   => "documents.forms.revenue_invoice.tax_rate",
                               "description" => "Tax Rate (%)" }
      }.freeze,

      "receipt" => {
        "vendor_name"   => { "type" => "string", "position" => 1,
                             "label_key"   => "documents.forms.receipt.vendor_name",
                             "description" => "Vendor Name" },
        "vendor_nif"    => { "type" => "string", "position" => 2,
                             "label_key"   => "documents.forms.receipt.vendor_nif",
                             "description" => "Vendor NIF" },
        "document_date" => { "type" => "date",   "position" => 3,
                             "label_key"   => "documents.forms.receipt.document_date",
                             "description" => "Document Date" },
        "receipt_number" => { "type" => "string", "position" => 4,
                              "label_key"   => "documents.forms.receipt.receipt_number",
                              "description" => "Receipt Number" },
        "amount_cents"  => { "type" => "money",  "position" => 5,
                             "label_key"   => "documents.forms.receipt.amount_cents",
                             "description" => "Amount (cents)" },
        "payment_method" => { "type" => "enum",  "position" => 6,
                              "values"      => PAYMENT_METHODS,
                              "label_key"   => "documents.forms.receipt.payment_method",
                              "description" => "Payment Method" }
      }.freeze,

      "bank_statement" => {
        "bank_name"      => { "type" => "string", "position" => 1,
                              "label_key"   => "documents.forms.bank_statement.bank_name",
                              "description" => "Bank Name" },
        "account_number" => { "type" => "string", "position" => 2,
                              "label_key"   => "documents.forms.bank_statement.account_number",
                              "description" => "Account Number" },
        "period_start"   => { "type" => "date",   "position" => 3,
                              "label_key"   => "documents.forms.bank_statement.period_start",
                              "description" => "Period Start" },
        "period_end"     => { "type" => "date",   "position" => 4,
                              "label_key"   => "documents.forms.bank_statement.period_end",
                              "description" => "Period End" },
        "opening_balance_cents" => { "type" => "money", "position" => 5,
                                     "label_key"   => "documents.forms.bank_statement.opening_balance",
                                     "description" => "Opening Balance (cents)" },
        "closing_balance_cents" => { "type" => "money", "position" => 6,
                                     "label_key"   => "documents.forms.bank_statement.closing_balance",
                                     "description" => "Closing Balance (cents)" },
        "currency"       => { "type" => "string", "position" => 7,
                              "label_key"   => "documents.forms.bank_statement.currency",
                              "description" => "Currency" }
      }.freeze,

      "other" => {
        "vendor_name"   => { "type" => "string", "position" => 1,
                             "label_key"   => "documents.forms.other.entity_name",
                             "description" => "Entity Name" },
        "document_date" => { "type" => "date",   "position" => 2,
                             "label_key"   => "documents.forms.other.document_date",
                             "description" => "Document Date" }
      }.freeze
    }.freeze

    def self.for(name)
      ALL[name.to_s]
    end
  end
end
