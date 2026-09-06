# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loan, type: :model do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  def build_loan(**attrs)
    create(:loan, workspace: workspace, created_by: user, **attrs)
  end

  describe "validations" do
    it "requires lender" do
      loan = build(:loan, lender: "")
      expect(loan).not_to be_valid
      expect(loan.errors[:lender]).not_to be_empty
    end

    it "requires positive principal_cents" do
      loan = build(:loan, principal_cents: 0)
      expect(loan).not_to be_valid
    end

    it "requires positive instalment_cents" do
      loan = build(:loan, instalment_cents: 0)
      expect(loan).not_to be_valid
    end

    it "requires positive term_months" do
      loan = build(:loan, term_months: 0)
      expect(loan).not_to be_valid
    end

    it "requires first_instalment_on" do
      loan = build(:loan, first_instalment_on: nil)
      expect(loan).not_to be_valid
    end

    it "requires 3-letter currency" do
      loan = build(:loan, currency: "EURO")
      expect(loan).not_to be_valid
    end

    it "is valid with correct attributes" do
      loan = build(:loan, workspace: workspace, created_by: user)
      expect(loan).to be_valid
    end
  end

  describe "#paid_instalments" do
    it "includes paid and unverified instalments" do
      loan = build_loan
      Loans::Schedule.build!(loan)
      loan.instalments.order(:number).first(3).each { |i| i.update!(status: :unverified) }
      loan.instalments.find_by(number: 4).update!(status: :paid)
      loan.reload
      expect(loan.paid_instalments.map(&:number)).to contain_exactly(1, 2, 3, 4)
    end
  end

  describe "#paid_count" do
    it "counts paid and unverified" do
      loan = build_loan
      Loans::Schedule.build!(loan)
      loan.instalments.first(2).each { |i| i.update!(status: :paid) }
      loan.reload
      expect(loan.paid_count).to eq(2)
    end
  end

  describe "#remaining_cents" do
    it "is remaining_count times instalment_cents" do
      loan = build_loan(term_months: 5, instalment_cents: 10_000)
      Loans::Schedule.build!(loan)
      loan.instalments.first(2).each { |i| i.update!(status: :paid) }
      loan.reload
      expect(loan.remaining_cents).to eq(3 * 10_000)
    end
  end

  describe "#next_expected" do
    it "returns the earliest expected instalment" do
      loan = build_loan(term_months: 3)
      Loans::Schedule.build!(loan)
      expect(loan.next_expected.number).to eq(1)
    end
  end

  describe "#ends_on" do
    it "returns the last instalment date" do
      loan = build_loan(term_months: 3, first_instalment_on: Date.new(2024, 1, 5))
      Loans::Schedule.build!(loan)
      expect(loan.ends_on).to eq(Date.new(2024, 3, 5))
    end
  end

  describe "#progress_pct" do
    it "returns correct percentage" do
      loan = build_loan(term_months: 10, instalment_cents: 10_000)
      Loans::Schedule.build!(loan)
      loan.instalments.first(3).each { |i| i.update!(status: :paid) }
      loan.reload
      expect(loan.progress_pct).to be_within(0.01).of(30.0)
    end
  end
end
