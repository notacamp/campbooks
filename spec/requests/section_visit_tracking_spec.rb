require "rails_helper"

# The TracksSectionVisit concern stamps User#section_seen_at on the way into each
# section's landing view, clearing its nav attention dot. One macro, wired across
# the section controllers — a representative few exercised here.
RSpec.describe "Section visit tracking", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  def expect_visit_recorded(section)
    user.mark_section_seen!(section, at: 1.hour.ago)
    yield
    expect(user.reload.seen_section_at(section)).to be > 5.minutes.ago
  end

  it "stamps :documents when visiting the documents index" do
    expect_visit_recorded(:documents) { get documents_path }
  end

  it "stamps :calendar when visiting the calendar" do
    expect_visit_recorded(:calendar) { get calendar_path }
  end

  it "stamps :calendar when visiting the reminders index" do
    expect_visit_recorded(:calendar) { get reminders_path }
  end

  it "stamps :scout when visiting Scout" do
    expect_visit_recorded(:scout) { get scout_path }
  end

  it "stamps :home when visiting the home feed" do
    expect_visit_recorded(:home) { get root_path }
  end
end
