# frozen_string_literal: true

require "rails_helper"

FIXTURES = File.join(Rails.root, "spec/fixtures/files/bank_statements")

RSpec.describe Reconciliations::CsvParser, type: :service do
  def fixture(name)
    File.binread(File.join(FIXTURES, name))
  end

  def parse(name)
    described_class.new(fixture(name)).call
  end

  # ── Fixture 1: semicolon delimiter + EU decimals + dd-mm-yyyy ───────────────
  describe "Millennium-style CSV (semicolons, EU decimals, dd-mm-yyyy)" do
    subject(:rows) { parse("millennium_semicolon.csv") }

    it "returns 3 data rows (skips footer rows)" do
      expect(rows.size).to eq(3)
    end

    it "parses the first date correctly" do
      expect(rows[0][:booked_on]).to eq(Date.new(2024, 1, 15))
    end

    it "parses EU decimal as signed cents (debit)" do
      expect(rows[0][:amount_cents]).to eq(-4590)
    end

    it "parses EU decimal as credit cents" do
      expect(rows[1][:amount_cents]).to eq(120_000)
    end

    it "assigns sequential positions" do
      expect(rows.map { |r| r[:position] }).to eq([ 0, 1, 2 ])
    end

    it "captures description" do
      expect(rows[0][:description]).to eq("Pagamento eletricidade")
    end
  end

  # ── Fixture 2: separate Débito / Crédito columns ───────────────────────────
  describe "Debit/credit column layout" do
    subject(:rows) { parse("debit_credit_columns.csv") }

    it "returns 3 rows" do
      expect(rows.size).to eq(3)
    end

    it "maps debit column to negative amount" do
      expect(rows[0][:amount_cents]).to eq(-85_000)
    end

    it "maps credit column to positive amount" do
      expect(rows[1][:amount_cents]).to eq(250_000)
    end

    it "maps beneficiario to counterparty" do
      expect(rows[0][:counterparty]).to eq("Imobiliária Central")
    end
  end

  # ── Fixture 3: comma-delimited US decimals, signed single amount ────────────
  describe "US comma-delimited CSV" do
    subject(:rows) { parse("us_comma_signed.csv") }

    it "returns 3 rows" do
      expect(rows.size).to eq(3)
    end

    it "parses US decimal amount correctly" do
      expect(rows[0][:amount_cents]).to eq(-12_550)
    end

    it "maps payee column to counterparty" do
      expect(rows[0][:counterparty]).to eq("Whole Foods")
    end
  end

  # ── Fixture 4: direction column layout ─────────────────────────────────────
  describe "Direction column (D/C) layout" do
    subject(:rows) { parse("direction_column.csv") }

    it "returns 3 rows" do
      expect(rows.size).to eq(3)
    end

    it "marks D as debit (negative)" do
      expect(rows[0][:amount_cents]).to be < 0
    end

    it "marks C as credit (positive)" do
      expect(rows[1][:amount_cents]).to be > 0
    end

    it "maps tiers to counterparty" do
      expect(rows[0][:counterparty]).to include("Agence")
    end
  end

  # ── Fixture 5: Latin-1 encoding with accents ────────────────────────────────
  describe "Latin-1 encoded file" do
    subject(:rows) { parse("latin1_accents.csv") }

    it "parses without encoding errors" do
      expect { rows }.not_to raise_error
    end

    it "returns 2 rows" do
      expect(rows.size).to eq(2)
    end

    it "properly decodes accented characters" do
      expect(rows[0][:description]).to include("empresa")
    end
  end

  # ── UTF-8 BOM file ──────────────────────────────────────────────────────────
  describe "UTF-8 BOM file" do
    subject(:rows) { parse("utf8_bom.csv") }

    it "strips BOM and parses correctly" do
      expect(rows.size).to eq(2)
    end
  end

  # ── Fixture 6: garbage file → ParseError ────────────────────────────────────
  describe "Garbage file" do
    it "raises ParseError" do
      expect { parse("garbage.csv") }.to raise_error(Reconciliations::ParseError)
    end
  end

  # ── Edge cases ──────────────────────────────────────────────────────────────
  describe "parentheses negative" do
    it "parses (45,90) as -4590 cents" do
      csv = "Data;Descricao;Valor\n15-01-2024;Fee;(45,90)\n"
      rows = described_class.new(csv).call
      expect(rows[0][:amount_cents]).to eq(-4590)
    end
  end

  describe "trailing minus" do
    it "parses '87,50-' as -8750 cents" do
      csv = "Date;Description;Amount\n15-01-2024;Something;87,50-\n"
      rows = described_class.new(csv).call
      expect(rows[0][:amount_cents]).to eq(-8750)
    end
  end
end
