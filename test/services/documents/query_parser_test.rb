# frozen_string_literal: true

require "test_helper"

module Documents
  class QueryParserTest < ActiveSupport::TestCase
    def parse(q)
      Documents::QueryParser.parse(q)
    end

    test "a blank query yields all-nil hints" do
      r = parse("")
      assert_nil r.document_type
      assert_nil r.number
      assert_nil r.counterparty
      assert_equal "", r.cleaned_query
    end

    # --- document type (EN + PT) ---

    test "detects expense_invoice from EN and PT invoice keywords" do
      assert_equal "expense_invoice", parse("invoice from EDP").document_type
      assert_equal "expense_invoice", parse("fatura da EDP").document_type
    end

    test "detects the unambiguous single-word types in both languages" do
      assert_equal "receipt", parse("recibo da EDP").document_type
      assert_equal "receipt", parse("all payment receipts").document_type
      assert_equal "contract", parse("the contract with Acme").document_type
      assert_equal "bank_statement", parse("extrato bancário").document_type
      assert_equal "insurance_policy", parse("seguro auto").document_type
      assert_equal "certificate", parse("certidão permanente").document_type
      assert_equal "credit_note", parse("nota de crédito").document_type
    end

    test "returns nil document_type when two distinct types match (ambiguous)" do
      assert_nil parse("fatura recibo").document_type
      assert_nil parse("contract and receipt").document_type
    end

    # --- numbers ---

    test "extracts Portuguese fiscal codes verbatim" do
      assert_equal "FT 2024/123", parse("invoice FT 2024/123").number
      assert_equal "FR-7/2025", parse("receipt FR-7/2025").number
    end

    test "extracts a number that follows a document noun" do
      assert_equal "12345", parse("invoice 12345").number
      assert_equal "998877", parse("apólice 998877").number
      assert_equal "2024/001", parse("fatura nº 2024/001").number
    end

    test "extracts a bare slashed number" do
      assert_equal "2023/45", parse("nota de crédito 2023/45").number
    end

    # --- counterparty ---

    test "extracts a labelled counterparty" do
      assert_equal "Acme Lda", parse("the contract with company Acme Lda").counterparty
      assert_equal "Acme", parse("all payment receipts to company Acme").counterparty
    end

    test "extracts a quoted counterparty and strips trailing punctuation" do
      assert_equal "Acme, Lda", parse(%q(contract "Acme, Lda.")).counterparty
    end

    test "does not guess a counterparty without an explicit cue" do
      assert_nil parse("invoice from EDP").counterparty
      assert_nil parse("the contract with Acme").counterparty
    end

    # --- cleaned query ---

    test "strips framing lead-ins from the embedded query" do
      assert_equal "invoice FT 2024/123", parse("I'm looking for invoice FT 2024/123").cleaned_query
    end

    test "leaves a query without a lead-in unchanged" do
      assert_equal "contract with Acme", parse("contract with Acme").cleaned_query
    end
  end
end
