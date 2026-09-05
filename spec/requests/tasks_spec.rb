# frozen_string_literal: true

require "rails_helper"

# The Tasks pages retired into Time (asks); /tasks and /tasks/:id survive only as
# redirects so old notification/digest links keep working.
RSpec.describe "Tasks (redirects to Time)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    allow(Features).to receive(:tasks?).and_return(true)
    sign_in(user)
  end

  it "GET /tasks redirects to /time" do
    get tasks_path
    expect(response).to redirect_to(time_path)
  end

  it "GET /tasks/:id redirects to /time on the ask's day when it is dated" do
    zone = user.effective_time_zone
    task = workspace.tasks.create!(title: "Dated", status: :todo, priority: :normal,
                                   due_at: Time.utc(2026, 9, 11, 9, 30))
    get task_path(task)

    expect(response).to redirect_to(time_path(date: task.due_at.in_time_zone(zone).to_date.iso8601))
  end

  it "GET /tasks/:id for an undated ask redirects to /time" do
    task = workspace.tasks.create!(title: "Undated", status: :todo, priority: :normal)
    get task_path(task)

    expect(response).to redirect_to(time_path)
  end

  it "GET /tasks/:id for an inaccessible ask still redirects to /time" do
    foreign = create(:workspace).tasks.create!(title: "Theirs", status: :todo, priority: :normal)
    get task_path(foreign)

    expect(response).to redirect_to(time_path)
  end

  it "404s when the tasks readiness flag is off" do
    allow(Features).to receive(:tasks?).and_return(false)
    get tasks_path
    expect(response).to have_http_status(:not_found)
  end
end
