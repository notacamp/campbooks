# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Asks", type: :request do
  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } } # Monday

  let(:workspace) { create(:workspace, entitlement_overrides: { "tasks" => { "allowed" => true } }) }
  let(:user) { create(:user, workspace: workspace) }
  let(:turbo) { { "Accept" => "text/vnd.turbo-stream.html" } }

  before { allow(Features).to receive(:tasks?).and_return(true) }

  def ask(**attrs)
    workspace.tasks.create!({ title: "Countersign Acme", status: :todo, priority: :normal }.merge(attrs))
  end

  context "signed in, tasks enabled and entitled" do
    before { sign_in(user) }

    it "hold holds Scout's slot and re-renders the agenda" do
      task = ask
      post hold_ask_path(task), headers: turbo

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("time_agenda")
      expect(FocusBlock.for_task(task).held).to be_present
      expect(task.reload).to be_todo
    end

    it "hold returns an error toast when no slot fits before the deadline" do
      task = ask(due_at: 90.minutes.from_now)
      post hold_ask_path(task), headers: turbo

      expect(response).to have_http_status(:unprocessable_entity)
      expect(FocusBlock.for_task(task)).to be_empty
    end

    it "schedule sets the due date from a preset and accepts the ask" do
      task = ask(status: :suggested, ai_suggested: true)
      patch schedule_ask_path(task), params: { on: "friday" }, headers: turbo

      expect(response).to have_http_status(:ok)
      expect(task.reload.due_at.to_date).to eq(Date.new(2026, 9, 11))
      expect(task).to be_todo
    end

    it "schedule accepts an ISO date" do
      task = ask
      patch schedule_ask_path(task), params: { on: "2026-09-11" }, headers: turbo

      expect(task.reload.due_at.to_date.iso8601).to eq("2026-09-11")
    end

    it "snooze snoozes for a week" do
      task = ask
      post snooze_ask_path(task), headers: turbo

      expect(response).to have_http_status(:ok)
      expect(task.reload.snoozed?).to be(true)
    end

    it "done completes the ask" do
      task = ask
      patch done_ask_path(task), headers: turbo

      expect(task.reload).to be_done
    end

    it "dismiss cancels a suggested ask" do
      task = ask(status: :suggested, ai_suggested: true)
      post dismiss_ask_path(task), headers: turbo

      expect(task.reload).to be_cancelled
    end

    it "falls back to an HTML redirect to /time" do
      task = ask
      patch done_ask_path(task)

      expect(response).to redirect_to(time_path)
      expect(task.reload).to be_done
    end

    it "404s an ask in another workspace (no existence leak)" do
      foreign = create(:workspace).tasks.create!(title: "Theirs", status: :todo, priority: :normal)
      patch done_ask_path(foreign), headers: turbo

      expect(response).to have_http_status(:not_found)
      expect(foreign.reload).not_to be_done
    end
  end

  it "404s every action when the tasks readiness flag is off" do
    task = ask
    allow(Features).to receive(:tasks?).and_return(false)
    sign_in(user)

    patch done_ask_path(task), headers: turbo
    expect(response).to have_http_status(:not_found)
    expect(task.reload).not_to be_done
  end

  it "blocks a mutating action when the workspace is not entitled to tasks" do
    ws = create(:workspace) # default plan: tasks not allowed
    gated_user = create(:user, workspace: ws)
    task = ws.tasks.create!(title: "Gated", status: :todo, priority: :normal)
    sign_in(gated_user)

    patch done_ask_path(task), headers: turbo

    expect(task.reload).not_to be_done
  end
end
