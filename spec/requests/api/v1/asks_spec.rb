# frozen_string_literal: true

require "rails_helper"

# The /api/v1/asks routes are a thin alias of /api/v1/tasks (same controller,
# scopes and payloads). These specs prove the alias responds like tasks.
RSpec.describe "API v1 asks (tasks alias)", type: :request do
  let(:workspace) { create(:workspace, entitlement_overrides: { "tasks" => { "allowed" => true } }) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers = api_auth_headers(workspace: workspace, user: user, scopes: "tasks:read")
  def write_headers = api_auth_headers(workspace: workspace, user: user, scopes: "tasks:write")

  it "lists asks via the alias" do
    workspace.tasks.create!(title: "Alias list", status: :todo, priority: :normal)

    get api_v1_asks_path, headers: read_headers

    expect(response).to have_http_status(:ok)
    expect(response.parsed_body["data"].map { |t| t["title"] }).to include("Alias list")
  end

  it "shows an ask via the alias" do
    task = workspace.tasks.create!(title: "Alias show", status: :todo, priority: :normal)

    get api_v1_ask_path(task), headers: read_headers

    expect(response).to have_http_status(:ok)
    expect(response.parsed_body["data"]["id"]).to eq(task.id)
  end

  it "creates an ask via the alias" do
    post api_v1_asks_path, params: { title: "New alias ask" }, headers: write_headers

    expect(response).to have_http_status(:created)
    expect(response.parsed_body["data"]["title"]).to eq("New alias ask")
  end

  it "completes an ask via the alias member route" do
    task = workspace.tasks.create!(title: "Finish me", status: :todo, priority: :normal)

    patch complete_api_v1_ask_path(task), headers: write_headers

    expect(response).to have_http_status(:ok)
    expect(task.reload).to be_done
  end

  it "rejects a call without the tasks scope (403 insufficient_scope)" do
    get api_v1_asks_path, headers: api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")

    expect(response).to have_http_status(:forbidden)
    expect(response.parsed_body.dig("error", "code")).to eq("insufficient_scope")
  end
end
