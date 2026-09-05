# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::TimePage::DayNote, type: :component do
  let(:user) { create(:user) }
  let(:zone) { user.effective_time_zone }

  def note(**attrs)
    Time::DayNote::Result.new(**{
      date: Date.current, meetings_count: 0, deadlines_count: 0, first_deadline_title: nil,
      focus: nil, late_obligation: nil, undated_count: 0
    }.merge(attrs))
  end

  def render_note(result)
    ApplicationController.render(described_class.new(note: result, zone: zone), layout: false)
  end

  it "appends the plural undated-count sentence" do
    expect(render_note(note(undated_count: 2))).to include("2 asks have no date yet.")
  end

  it "uses the singular for one undated ask" do
    expect(render_note(note(undated_count: 1))).to include("One ask has no date yet.")
  end

  it "omits the sentence when there are no undated asks" do
    expect(render_note(note(undated_count: 0))).not_to include("no date yet")
  end
end
