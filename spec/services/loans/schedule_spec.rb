# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loans::Schedule, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:loan) do
    create(:loan, workspace: workspace, created_by: user,
                  first_instalment_on: Date.new(2024, 1, 5),
                  instalment_cents: 78_000, term_months: 6)
  end

  describe ".build!" do
    it "creates term_months instalments" do
      described_class.build!(loan)
      expect(loan.instalments.count).to eq(6)
    end

    it "numbers instalments 1..term_months" do
      described_class.build!(loan)
      expect(loan.instalments.order(:number).pluck(:number)).to eq((1..6).to_a)
    end

    it "sets expected_on via Date#>> so months advance correctly" do
      described_class.build!(loan)
      dates = loan.instalments.order(:number).pluck(:expected_on)
      expect(dates.first).to eq(Date.new(2024, 1, 5))
      expect(dates.last).to  eq(Date.new(2024, 6, 5))
    end

    it "sets amount_cents to instalment_cents" do
      described_class.build!(loan)
      expect(loan.instalments.pluck(:amount_cents).uniq).to eq([ 78_000 ])
    end

    it "sets all statuses to :expected" do
      described_class.build!(loan)
      expect(loan.instalments.pluck(:status).uniq).to eq([ "expected" ])
    end

    it "handles end-of-month dates without error (Jan 31 -> Feb 28)" do
      loan_eom = create(:loan, workspace: workspace, created_by: user,
                               first_instalment_on: Date.new(2024, 1, 31),
                               term_months: 3)
      described_class.build!(loan_eom)
      dates = loan_eom.instalments.order(:number).pluck(:expected_on)
      expect(dates[1]).to eq(Date.new(2024, 2, 29)) # 2024 is a leap year
      expect(dates[2]).to eq(Date.new(2024, 3, 31))
    end
  end

  describe ".rebuild!" do
    before { described_class.build!(loan) }

    it "preserves paid instalments" do
      loan.instalments.find_by!(number: 1).update!(status: :paid, amount_cents: 78_000)
      described_class.rebuild!(loan)
      expect(loan.instalments.find_by!(number: 1).status).to eq("paid")
    end

    it "preserves unverified instalments" do
      loan.instalments.find_by!(number: 2).update!(status: :unverified)
      described_class.rebuild!(loan)
      expect(loan.instalments.find_by!(number: 2).status).to eq("unverified")
    end

    it "regenerates expected instalments with updated amount" do
      loan.update!(instalment_cents: 80_000)
      described_class.rebuild!(loan)
      expected = loan.instalments.where(status: :expected)
      expect(expected.pluck(:amount_cents).uniq).to eq([ 80_000 ])
    end

    it "keeps total count equal to term_months" do
      described_class.rebuild!(loan)
      expect(loan.instalments.count).to eq(6)
    end
  end
end
