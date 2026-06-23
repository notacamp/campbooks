require "rails_helper"

RSpec.describe Feed::Sources::Reminder do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  subject(:source) { described_class.new(user) }

  def reminder(**attrs)
    create(:reminder, { workspace: workspace, source: create(:document, workspace: workspace) }.merge(attrs))
  end

  describe "#candidates" do
    it "surfaces pending, high-confidence reminders within the horizon" do
      good = reminder(confidence: 0.9, due_at: 2.days.from_now)
      low  = reminder(confidence: 0.4, due_at: 2.days.from_now)
      reminder(confidence: 0.9, status: :confirmed)

      subjects = source.candidates.map { |c| c[:subject] }
      expect(subjects).to include(good)
      expect(subjects).not_to include(low)
    end
  end

  describe "#still_valid?" do
    it "is valid for pending and invalid once acted on" do
      r = reminder
      expect(source.still_valid?(nil, r)).to be(true)
      r.update!(status: :dismissed)
      expect(source.still_valid?(nil, r)).to be(false)
    end

    it "is invalid for a nil subject (deleted record)" do
      expect(source.still_valid?(nil, nil)).to be(false)
    end
  end
end
