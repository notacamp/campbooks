require "rails_helper"

RSpec.describe "Settings::Integrations::Connections", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  it "lists the workspace's connections" do
    create(:connection, workspace: workspace, name: "Stripe")

    get settings_integrations_connections_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Stripe")
  end

  it "renders the new form" do
    get new_settings_integrations_connection_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Base URL")
  end

  it "surfaces an API Connections card on the integrations index" do
    get settings_integrations_root_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("API Connections")
  end

  it "creates a connection and stores the secret encrypted" do
    expect {
      post settings_integrations_connections_path, params: {
        connection: { name: "Stripe", base_url: "https://api.stripe.com", auth_type: "bearer", auth_secret: "sk_test" }
      }
    }.to change(workspace.connections, :count).by(1)

    connection = workspace.connections.last
    expect(connection.auth_secret).to eq("sk_test")
    raw = ActiveRecord::Base.connection.select_value("SELECT auth_secret FROM connections WHERE id = '#{connection.id}'")
    expect(raw).not_to include("sk_test")
  end

  it "rejects a bearer connection without a secret" do
    post settings_integrations_connections_path, params: {
      connection: { name: "X", base_url: "https://api.x.com", auth_type: "bearer", auth_secret: "" }
    }

    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "keeps the existing secret when the field is left blank on update" do
    connection = create(:connection, :bearer, workspace: workspace)

    patch settings_integrations_connection_path(connection), params: {
      connection: { name: "Renamed", auth_type: "bearer", auth_secret: "" }
    }

    expect(connection.reload.name).to eq("Renamed")
    expect(connection.auth_secret).to eq("tok_secret")
  end

  it "deletes a connection" do
    connection = create(:connection, workspace: workspace)

    expect {
      delete settings_integrations_connection_path(connection)
    }.to change(workspace.connections, :count).by(-1)
  end

  it "scopes to the current workspace (cannot touch another workspace's connection)" do
    foreign = create(:connection, workspace: create(:workspace))

    expect {
      delete settings_integrations_connection_path(foreign)
    }.not_to change(Connection, :count)
    expect(response).to have_http_status(:not_found)
  end
end
