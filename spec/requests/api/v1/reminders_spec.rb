require "rails_helper"

RSpec.describe "API v1 reminders", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "reminders:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "reminders:write")
  end

  describe "GET /api/v1/reminders" do
    it "lists accessible reminders ordered soonest first" do
      create(:reminder, workspace: workspace, title: "Later", due_at: 5.days.from_now)
      create(:reminder, workspace: workspace, title: "Sooner", due_at: 1.day.from_now)

      get api_v1_reminders_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].map { |r| r["title"] }).to eq(%w[Sooner Later])
    end

    it "filters by status" do
      create(:reminder, workspace: workspace, status: :pending)
      create(:reminder, :snoozed, workspace: workspace)

      get api_v1_reminders_path, params: { status: "snoozed" }, headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data.size).to eq(1)
      expect(data.first["status"]).to eq("snoozed")
    end

    it "does not leak another workspace's reminders" do
      create(:reminder, workspace: create(:workspace))

      get api_v1_reminders_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/reminders/:id" do
    it "returns the reminder with detail fields" do
      reminder = create(:reminder, workspace: workspace)

      get api_v1_reminder_path(reminder), headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(reminder.id)
      expect(data).to have_key("justification")
    end

    it "404s across workspaces" do
      other_reminder = create(:reminder, workspace: create(:workspace))

      get api_v1_reminder_path(other_reminder), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/reminders/:id/confirm" do
    it "returns 200 with the confirmed reminder and calendar_event_id" do
      reminder = create(:reminder, workspace: workspace)
      allow(Reminders::Confirm).to receive(:call)
        .and_return(double("ConfirmResult", success?: true, calendar_event: nil, error: nil))

      post confirm_api_v1_reminder_path(reminder), headers: write_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(reminder.id)
      expect(data).to have_key("calendar_event_id")
    end

    it "422s when confirm fails" do
      reminder = create(:reminder, workspace: workspace)
      allow(Reminders::Confirm).to receive(:call)
        .and_return(double("ConfirmResult", success?: false, calendar_event: nil, error: "No calendar"))

      post confirm_api_v1_reminder_path(reminder), headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("confirm_failed")
    end

    it "403s with only the read scope" do
      reminder = create(:reminder, workspace: workspace)

      post confirm_api_v1_reminder_path(reminder), headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/reminders/:id/dismiss" do
    it "flips status to dismissed" do
      reminder = create(:reminder, workspace: workspace, status: :pending)

      post dismiss_api_v1_reminder_path(reminder), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(reminder.reload.status).to eq("dismissed")
      expect(response.parsed_body.dig("data", "status")).to eq("dismissed")
    end
  end

  describe "POST /api/v1/reminders/:id/snooze" do
    it "sets snoozed_until to the given time" do
      reminder = create(:reminder, workspace: workspace)
      target = 2.weeks.from_now

      post snooze_api_v1_reminder_path(reminder), params: { until: target.iso8601 }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(reminder.reload.status).to eq("snoozed")
      expect(reminder.snoozed_until).to be_within(1.second).of(target)
    end

    it "defaults snoozed_until to 1 week when no until param is given" do
      reminder = create(:reminder, workspace: workspace)

      post snooze_api_v1_reminder_path(reminder), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(reminder.reload.snoozed_until).to be_within(5.seconds).of(1.week.from_now)
    end
  end
end
