# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Calendar Visibilities API (app)", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) do
    create(:calendar_account, workspace: workspace).tap do |acc|
      create(:calendar_account_user, calendar_account: acc, user: user,
                                     can_read: true, owner: true)
    end
  end
  let!(:calendar) { create(:calendar, calendar_account: calendar_account) }

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  describe "PATCH /api/app/calendar_visibilities/:id" do
    it "hides a calendar and returns hidden: true" do
      patch "/api/app/calendar_visibilities/#{calendar.id}",
            params: { hidden: true },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "hidden")).to be(true)
      expect(user.reload.hidden_calendar_ids).to include(calendar.id.to_s)
    end

    it "un-hides a previously hidden calendar" do
      user.set_calendar_hidden!(calendar, true)

      patch "/api/app/calendar_visibilities/#{calendar.id}",
            params: { hidden: false },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "hidden")).to be(false)
      expect(user.reload.hidden_calendar_ids).not_to include(calendar.id.to_s)
    end

    it "is idempotent — hiding twice stays hidden" do
      patch "/api/app/calendar_visibilities/#{calendar.id}",
            params: { hidden: true }, headers: api_app_headers(user)
      patch "/api/app/calendar_visibilities/#{calendar.id}",
            params: { hidden: true }, headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(user.reload.hidden_calendar_ids.count(calendar.id.to_s)).to eq(1)
    end

    it "404s for a calendar not readable by the user" do
      other_ws  = create(:workspace)
      other_acc = create(:calendar_account, workspace: other_ws)
      other_cal = create(:calendar, calendar_account: other_acc)

      patch "/api/app/calendar_visibilities/#{other_cal.id}",
            params: { hidden: true },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end

    it "401s without a token" do
      patch "/api/app/calendar_visibilities/#{calendar.id}", params: { hidden: true }
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
