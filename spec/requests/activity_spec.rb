require "rails_helper"

RSpec.describe "Activity feed", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  it "renders the workspace timeline with event labels" do
    create(:event, workspace: workspace, name: "document.approved",
                   payload: { "filename" => "invoice.pdf" })
    create(:event, workspace: workspace, name: "contact.starred",
                   payload: { "name" => "Maple Lodge" })

    get activity_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Document approved")
    expect(response.body).to include("Contact starred")
    expect(response.body).to include("Activity") # page heading
  end

  it "filters by group" do
    create(:event, workspace: workspace, name: "document.approved", payload: { "filename" => "a.pdf" })
    create(:event, workspace: workspace, name: "contact.starred", payload: { "name" => "X" })

    get activity_path(group: "documents")

    expect(response.body).to include("Document approved")
    expect(response.body).not_to include("Contact starred")
  end

  it "scopes to the current workspace" do
    other = create(:workspace)
    create(:event, workspace: other, name: "document.approved", payload: { "filename" => "secret.pdf" })

    get activity_path

    expect(response.body).not_to include("secret.pdf")
  end

  it "hides email events for mailboxes the user cannot read" do
    hidden_account = create(:email_account, workspace: workspace)
    hidden_email = create(:email_message, email_account: hidden_account, subject: "Confidential")
    create(:event, workspace: workspace, name: "email.received", subject: hidden_email,
                   payload: { "subject" => "Confidential" })

    get activity_path

    expect(response.body).not_to include("Confidential")
  end

  it "serves pagination as a turbo stream" do
    create_list(:event, 2, workspace: workspace, name: "document.approved", payload: { "filename" => "a.pdf" })

    get activity_path(format: :turbo_stream)

    expect(response).to have_http_status(:ok)
    expect(response.media_type).to eq("text/vnd.turbo-stream.html")
  end
end
