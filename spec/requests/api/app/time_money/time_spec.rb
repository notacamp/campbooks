# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Time surface", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:headers)   { api_app_headers(user) }

  # ── GET /api/app/time ───────────────────────────────────────────────────────
  describe "GET /api/app/time" do
    it "returns 200 with items/undated/day_note/suggestions keys" do
      travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
        get "/api/app/time", headers: headers

        expect(response).to have_http_status(:ok)
        body = response.parsed_body["data"]
        expect(body).to include("items", "undated", "day_note", "suggestions")
        expect(body["items"]).to be_an(Array)
        expect(body["undated"]).to be_an(Array)
        expect(body["suggestions"]).to be_an(Array)
      end
    end

    it "401s without a token" do
      get "/api/app/time"
      expect(response).to have_http_status(:unauthorized)
    end

    it "accepts a ?date= param" do
      travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
        get "/api/app/time", params: { date: "2026-09-25" }, headers: headers
        expect(response).to have_http_status(:ok)
      end
    end

    context "with calendar events in the range" do
      it "includes the event in items" do
        travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
          account  = create(:calendar_account, workspace: workspace)
          CalendarAccountUser.create!(calendar_account: account, user: user, can_read: true, can_write: false, can_manage: false)
          calendar = create(:calendar, calendar_account: account, syncing: true)
          create(:calendar_event,
                 calendar:  calendar,
                 title:     "Standup",
                 start_at:  ::Time.zone.local(2026, 9, 20, 10, 0, 0),
                 end_at:    ::Time.zone.local(2026, 9, 20, 10, 30, 0),
                 all_day:   false,
                 status:    :confirmed)

          get "/api/app/time", headers: headers

          body  = response.parsed_body["data"]
          items = body["items"]
          expect(items.any? { |i| i["title"] == "Standup" && i["kind"] == "event" }).to be true
        end
      end
    end
  end

  # ── PATCH /api/app/account/time_zone ────────────────────────────────────────
  describe "PATCH /api/app/account/time_zone" do
    it "saves the time zone when blank" do
      user.update!(time_zone: nil)
      patch "/api/app/account/time_zone", params: { time_zone: "Europe/Lisbon" }, headers: headers

      expect(response).to have_http_status(:no_content)
      expect(user.reload.time_zone).to eq("Europe/Lisbon")
    end

    it "is a no-op when time_zone is already set" do
      user.update!(time_zone: "America/New_York")
      patch "/api/app/account/time_zone", params: { time_zone: "Europe/Lisbon" }, headers: headers

      expect(response).to have_http_status(:no_content)
      expect(user.reload.time_zone).to eq("America/New_York")
    end

    it "401s without a token" do
      patch "/api/app/account/time_zone", params: { time_zone: "UTC" }
      expect(response).to have_http_status(:unauthorized)
    end
  end

  # ── Ask mutations ────────────────────────────────────────────────────────────
  describe "Ask mutations" do
    before { allow(Features).to receive(:tasks?).and_return(true) }

    let(:task) do
      workspace.tasks.create!(
        title:      "Test ask",
        status:     :suggested,
        priority:   :normal,
        created_by: user
      )
    end

    describe "POST /api/app/asks/:id/schedule" do
      it "sets a due date and returns the updated ask" do
        travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
          patch "/api/app/asks/#{task.id}/schedule",
                params: { on: (::Date.current + 2).iso8601 }, headers: headers

          expect(response).to have_http_status(:ok)
          body = response.parsed_body["data"]
          expect(body["item"]["id"]).to eq(task.id)
          expect(task.reload.due_at).to be_present
        end
      end

      it "returns 404 for a task in another workspace" do
        other_task = create(:workspace).tasks.create!(
          title:    "Other ask",
          status:   :suggested,
          priority: :normal
        )
        patch "/api/app/asks/#{other_task.id}/schedule",
              params: { on: "2026-09-25" }, headers: headers

        expect(response).to have_http_status(:not_found)
      end
    end

    describe "POST /api/app/asks/:id/snooze" do
      it "snoozes the ask" do
        travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
          post "/api/app/asks/#{task.id}/snooze", headers: headers

          expect(response).to have_http_status(:ok)
          expect(task.reload.snoozed_until).to be_present
        end
      end
    end

    describe "POST /api/app/asks/:id/done" do
      it "marks the ask done" do
        post "/api/app/asks/#{task.id}/done", headers: headers

        expect(response).to have_http_status(:ok)
        expect(task.reload.status).to eq("done")
      end
    end

    describe "POST /api/app/asks/:id/dismiss" do
      it "cancels the ask" do
        post "/api/app/asks/#{task.id}/dismiss", headers: headers

        expect(response).to have_http_status(:ok)
        expect(task.reload.status).to eq("cancelled")
      end
    end
  end

  # ── FocusBlock mutations ─────────────────────────────────────────────────────
  describe "FocusBlock mutations" do
    let(:focus_block) do
      FocusBlock.create!(
        user:      user,
        workspace: workspace,
        title:     "Deep work",
        status:    :proposed,
        start_at:  ::Time.zone.local(2026, 9, 21, 10, 0, 0),
        end_at:    ::Time.zone.local(2026, 9, 21, 10, 45, 0)
      )
    end

    describe "PATCH /api/app/focus_blocks/:id/move" do
      it "reschedules the block" do
        travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
          patch "/api/app/focus_blocks/#{focus_block.id}/move",
                params: { start_at: "2026-09-22T10:00:00+01:00" }, headers: headers

          expect(response).to have_http_status(:ok)
          expect(focus_block.reload.status).to eq("moved")
        end
      end
    end

    describe "DELETE /api/app/focus_blocks/:id" do
      it "dismisses the block" do
        delete "/api/app/focus_blocks/#{focus_block.id}", headers: headers

        expect(response).to have_http_status(:ok)
        expect(focus_block.reload.status).to eq("dismissed")
      end

      it "404s for another user's block" do
        other_user = create(:user, workspace: workspace)
        other = FocusBlock.create!(
          user:      other_user,
          workspace: workspace,
          title:     "Other block",
          status:    :proposed,
          start_at:  ::Time.zone.local(2026, 9, 21, 10, 0, 0),
          end_at:    ::Time.zone.local(2026, 9, 21, 10, 45, 0)
        )
        delete "/api/app/focus_blocks/#{other.id}", headers: headers

        expect(response).to have_http_status(:not_found)
      end
    end
  end

  # ── Reminder mutations ────────────────────────────────────────────────────────
  describe "Reminder mutations" do
    let(:reminder) do
      create(:reminder, workspace: workspace,
             status: :pending,
             due_at: ::Time.zone.local(2026, 9, 25, 9, 0, 0),
             all_day: false)
    end

    describe "DELETE /api/app/reminders/:id" do
      it "dismisses the reminder" do
        delete "/api/app/reminders/#{reminder.id}", headers: headers

        expect(response).to have_http_status(:ok)
        expect(reminder.reload.status).to eq("dismissed")
      end

      it "404s for a reminder in another workspace" do
        other = create(:reminder, workspace: create(:workspace), status: :pending,
                       due_at: ::Time.zone.local(2026, 9, 25, 9, 0, 0))
        delete "/api/app/reminders/#{other.id}", headers: headers

        expect(response).to have_http_status(:not_found)
      end
    end

    describe "POST /api/app/reminders/:id/snooze" do
      it "snoozes the reminder" do
        travel_to ::Time.zone.local(2026, 9, 20, 9, 0, 0) do
          post "/api/app/reminders/#{reminder.id}/snooze", headers: headers

          expect(response).to have_http_status(:ok)
          expect(reminder.reload.status).to eq("snoozed")
          expect(reminder.reload.snoozed_until).to be_present
        end
      end
    end
  end
end
