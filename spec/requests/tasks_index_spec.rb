# frozen_string_literal: true

require "rails_helper"

# The tasks surface defaults to the status board; ?view=list shows the flat list.
# Also smoke-tests that the new-task form and a task's page render (they carry the
# PillMultiSelect assignee/tag pickers).
RSpec.describe "Tasks index views", type: :request do
  include ActionView::RecordIdentifier

  before do
    @workspace = Workspace.create!(name: "Tasks View WS")
    @user = @workspace.users.create!(
      name: "Viewer",
      email_address: "tasks-view-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    @task = @workspace.tasks.create!(title: "Board me", status: :todo, created_by: @user)
    sign_in(@user)
  end

  it "defaults to the board view" do
    with_env("ENABLE_TASKS" => "1") do
      get tasks_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include('data-controller="tasks-board"')
    end
  end

  it "shows the flat list with ?view=list" do
    with_env("ENABLE_TASKS" => "1") do
      get tasks_path(view: :list)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(dom_id(@task, :list_item))
    end
  end

  it "renders the new-task form" do
    with_env("ENABLE_TASKS" => "1") do
      get new_task_path

      expect(response).to have_http_status(:ok)
    end
  end

  it "renders a task's page" do
    with_env("ENABLE_TASKS" => "1") do
      get task_path(@task)

      expect(response).to have_http_status(:ok)
    end
  end
end
