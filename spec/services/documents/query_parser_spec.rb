# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::QueryParser do
  def parse(q)
    described_class.parse(q)
  end

  it "a blank query yields all-nil hints" do
    r = parse("")
    expect(r.document_type).to be_nil
    expect(r.number).to be_nil
    expect(r.counterparty).to be_nil
    expect(r.cleaned_query).to eq("")
  end

  # --- document type (EN + PT) ---

  it "detects expense_invoice from EN and PT invoice keywords" do
    expect(parse("invoice from EDP").document_type).to eq("expense_invoice")
    expect(parse("fatura da EDP").document_type).to eq("expense_invoice")
  end

  it "detects the unambiguous single-word types in both languages" do
    expect(parse("recibo da EDP").document_type).to eq("receipt")
    expect(parse("all payment receipts").document_type).to eq("receipt")
    expect(parse("the contract with Acme").document_type).to eq("contract")
    expect(parse("extrato bancário").document_type).to eq("bank_statement")
    expect(parse("seguro auto").document_type).to eq("insurance_policy")
    expect(parse("certidão permanente").document_type).to eq("certificate")
    expect(parse("nota de crédito").document_type).to eq("credit_note")
  end

  it "returns nil document_type when two distinct types match (ambiguous)" do
    expect(parse("fatura recibo").document_type).to be_nil
    expect(parse("contract and receipt").document_type).to be_nil
  end

  # --- numbers ---

  it "extracts Portuguese fiscal codes verbatim" do
    expect(parse("invoice FT 2024/123").number).to eq("FT 2024/123")
    expect(parse("receipt FR-7/2025").number).to eq("FR-7/2025")
  end

  it "extracts a number that follows a document noun" do
    expect(parse("invoice 12345").number).to eq("12345")
    expect(parse("apólice 998877").number).to eq("998877")
    expect(parse("fatura nº 2024/001").number).to eq("2024/001")
  end

  it "extracts a bare slashed number" do
    expect(parse("nota de crédito 2023/45").number).to eq("2023/45")
  end

  # --- counterparty ---

  it "extracts a labelled counterparty" do
    expect(parse("the contract with company Acme Lda").counterparty).to eq("Acme Lda")
    expect(parse("all payment receipts to company Acme").counterparty).to eq("Acme")
  end

  it "extracts a quoted counterparty and strips trailing punctuation" do
    expect(parse(%q(contract "Acme, Lda.")).counterparty).to eq("Acme, Lda")
  end

  it "does not guess a counterparty without an explicit cue" do
    expect(parse("invoice from EDP").counterparty).to be_nil
    expect(parse("the contract with Acme").counterparty).to be_nil
  end

  # --- cleaned query ---

  it "strips framing lead-ins from the embedded query" do
    expect(parse("I'm looking for invoice FT 2024/123").cleaned_query).to eq("invoice FT 2024/123")
  end

  it "leaves a query without a lead-in unchanged" do
    expect(parse("contract with Acme").cleaned_query).to eq("contract with Acme")
  end
end
