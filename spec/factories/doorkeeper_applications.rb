FactoryBot.define do
  # A Doorkeeper OAuth application = a customer's API client. created_by must
  # belong to the application's workspace — the acting-identity bridge in
  # Api::V1::BaseController asserts acting_user.workspace_id == workspace.id.
  factory :api_application, class: "Doorkeeper::Application" do
    sequence(:name) { |n| "API Client #{n}" }
    redirect_uri { "" }
    confidential { true }
    scopes { "emails:read" }

    workspace
    created_by { association(:user, workspace: workspace) }
  end
end
