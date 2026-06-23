require "rails_helper"

RSpec.describe "Reminders", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  def reminder
    create(:reminder, workspace: workspace, source: create(:document, workspace: workspace))
  end

  describe "GET /reminders" do
    it "renders for the authenticated user" do
      reminder
      get reminders_path
      expect(response).to have_http_status(:ok)
    end
  end

  describe "POST /reminders/:id/confirm" do
    it "confirms the reminder (no calendar → still confirmed)" do
      r = reminder
      post confirm_reminder_path(r)
      expect(r.reload).to be_confirmed
    end
  end

  describe "POST /reminders/:id/dismiss" do
    it "dismisses the reminder" do
      r = reminder
      post dismiss_reminder_path(r)
      expect(r.reload).to be_dismissed
    end
  end

  describe "POST /reminders/:id/snooze" do
    it "snoozes the reminder" do
      r = reminder
      post snooze_reminder_path(r)
      expect(r.reload).to be_snoozed
    end
  end

  it "404s for a reminder outside the user's workspace" do
    other = create(:reminder, workspace: create(:workspace))
    post dismiss_reminder_path(other)
    expect(response).to have_http_status(:not_found)
  end
end
