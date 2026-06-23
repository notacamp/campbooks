require "rails_helper"

RSpec.describe BugReport, type: :model do
  describe "associations" do
    it { is_expected.to belong_to(:workspace) }
    it { is_expected.to belong_to(:user) }
  end

  describe "validations" do
    it { is_expected.to validate_presence_of(:description) }
    it { is_expected.to validate_length_of(:description).is_at_most(5_000) }
  end

  describe "enums" do
    it {
      is_expected.to define_enum_for(:status)
        .with_values(open: 0, triaged: 1, resolved: 2, closed: 3)
    }
  end

  describe "#issue_title" do
    it "uses the first line of the description" do
      report = build(:bug_report, description: "Skim button is dead\nWhen I tap it nothing happens")
      expect(report.issue_title).to eq("Skim button is dead")
    end

    it "truncates a long single line to 80 characters" do
      report = build(:bug_report, description: "x" * 200)
      expect(report.issue_title.length).to be <= 80
      expect(report.issue_title).to end_with("…")
    end

    it "falls back to the record id when the body is blank" do
      report = build(:bug_report, description: "   ")
      report.id = 7
      expect(report.issue_title).to eq("Bug report #7")
    end
  end

  describe "#synced_to_github?" do
    it { expect(build(:bug_report, :synced)).to be_synced_to_github }
    it { expect(build(:bug_report, github_issue_number: nil)).not_to be_synced_to_github }
  end

  describe "#context" do
    it "reads a metadata value by string or symbol key" do
      report = build(:bug_report, metadata: { "viewport" => "375x812" })
      expect(report.context("viewport")).to eq("375x812")
      expect(report.context(:viewport)).to eq("375x812")
    end

    it "is nil-safe when metadata is not a hash" do
      report = build(:bug_report)
      report.metadata = nil
      expect(report.context("viewport")).to be_nil
    end
  end
end
