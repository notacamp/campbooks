# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Asks", type: :request do
  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } } # Monday

  let(:workspace) { create(:workspace) } # default (free) plan — asks need no entitlement
  let(:user) { create(:user, workspace: workspace) }
  let(:turbo) { { "Accept" => "text/vnd.turbo-stream.html" } }

  before { allow(Features).to receive(:tasks?).and_return(true) }

  def ask(**attrs)
    workspace.tasks.create!({ title: "Countersign Acme", status: :todo, priority: :normal }.merge(attrs))
  end

  context "signed in, tasks enabled" do
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

  it "allows a mutating action on any plan — asks are no longer entitlement-gated" do
    ws = create(:workspace) # default (free) plan; no tasks entitlement any more
    plan_user = create(:user, workspace: ws)
    task = ws.tasks.create!(title: "Core", status: :todo, priority: :normal)
    sign_in(plan_user)

    patch done_ask_path(task), headers: turbo

    expect(response).to have_http_status(:ok)
    expect(task.reload).to be_done
  end

  describe "return=people response" do
    let(:account) { create(:email_account, workspace: workspace, email_address: "me@biz.example") }
    let(:person) { create(:person, workspace: workspace) }
    let(:contact) do
      create(:contact, workspace: workspace, email_account: account, person: person,
             sender_kind: :person, sender_kind_source: "heuristic", email: "sofia@x.example")
    end
    let(:thread) { create(:email_thread, email_account: account) }
    let(:source_msg) do
      create(:email_message, email_account: account, contact: contact, email_thread: thread,
             from_address: contact.email, to_address: account.email_address,
             provider_folder_id: "INBOX", received_at: 3.days.ago)
    end

    before do
      create(:email_account_user, user: user, email_account: account, can_read: true)
      allow(Emails::InboxFolders).to receive(:ids_for).and_return(%w[INBOX])
      sign_in(user)
    end

    it "replaces the stand note when return=people is sent (turbo_stream)" do
      task = workspace.tasks.create!(title: "People return test", status: :todo,
                                     priority: :normal, created_by: user, source: source_msg)
      patch done_ask_path(task), params: { return: "people" }, headers: turbo

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("stand_note_person_#{person.id}")
    end

    it "falls back to redirect_back when return=people and turbo not present" do
      task = workspace.tasks.create!(title: "People return HTML", status: :todo,
                                     priority: :normal, created_by: user, source: source_msg)
      patch done_ask_path(task), params: { return: "people" }
      expect(response).to redirect_to(people_path)
    end
  end

  context "hand-off" do
    before { sign_in(user) }

    let(:member) { create(:user, workspace: workspace, name: "Ana Ng") }

    it "hands an ask to a member, notifies them, and re-renders the agenda" do
      task = ask
      post hand_off_ask_path(task), params: { user_id: member.id }, headers: turbo

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("time_agenda")
      expect(task.reload.handed_to).to eq(member)
      expect(member.notifications.where(category: :task).count).to eq(1)
    end

    it "404s when handing to yourself" do
      task = ask
      post hand_off_ask_path(task), params: { user_id: user.id }, headers: turbo

      expect(response).to have_http_status(:not_found)
      expect(task.reload.handed?).to be(false)
    end

    it "404s when handing to a non-member" do
      outsider = create(:user) # a different workspace
      task = ask
      post hand_off_ask_path(task), params: { user_id: outsider.id }, headers: turbo

      expect(response).to have_http_status(:not_found)
    end

    it "lets the assigner take it back" do
      task = ask
      Asks::HandOff.call(task, to: member, by: user)

      post take_back_ask_path(task), headers: turbo

      expect(response).to have_http_status(:ok)
      expect(task.reload.handed?).to be(false)
    end

    it "forbids another member from taking it back (404)" do
      task = ask
      Asks::HandOff.call(task, to: member, by: user)
      sign_in_as(create(:user, workspace: workspace))

      post take_back_ask_path(task), headers: turbo

      expect(response).to have_http_status(:not_found)
      expect(task.reload.handed?).to be(true)
    end

    it "lets a workspace admin take it back" do
      task = ask
      Asks::HandOff.call(task, to: member, by: user)
      sign_in_as(create(:user, workspace: workspace, role: :admin))

      post take_back_ask_path(task), headers: turbo

      expect(response).to have_http_status(:ok)
      expect(task.reload.handed?).to be(false)
    end
  end
end
