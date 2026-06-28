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

    # A public client (the CLI): no secret, PKCE-only, loopback redirect, and no
    # workspace/created_by — its identity comes from each token's resource owner.
    trait :public_client do
      confidential { false }
      redirect_uri { "http://127.0.0.1/callback\nurn:ietf:wg:oauth:2.0:oob" }
      workspace { nil }
      created_by { nil }
    end
  end
end
