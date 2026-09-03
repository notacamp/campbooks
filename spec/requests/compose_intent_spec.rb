require "rails_helper"

# GET /email_messages/new — bold layout "compose from intent". The Desk infers
# To/Subject from ?intent= / ?to= and renders them as inferred chips. Classic
# layout ignores the params entirely.
RSpec.describe "Compose from intent (Desk)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, layout_mode: :bold) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "me@example.com") }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    create(:contact, workspace: workspace, name: "Sofia Martins", email: "sofia@example.com")
    sign_in(user)
  end

  def to_field_value(body)
    body[/name="to_address"\s+value="([^"]*)"/, 1]
  end

  def subject_field_value(body)
    body[/name="subject"\s+value="([^"]*)"/, 1]
  end

  context "with the bold layout enabled" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "prefills an inferred recipient and subject from the intent" do
      get new_email_message_path(intent: "write to Sofia about the Q3 kickoff deck")

      expect(response).to have_http_status(:ok)
      expect(to_field_value(response.body)).to eq("Sofia Martins <sofia@example.com>")
      expect(subject_field_value(response.body)).to eq("The Q3 kickoff deck")
      # the To pill input is marked inferred; the subject chip shows the suffix
      expect(response.body).to include('data-contact-pill-input-inferred-value="true"')
      expect(response.body).to include("compose-engine-target=\"subjectInferred\"")
    end

    it "renders the intent input pre-filled with the note" do
      get new_email_message_path(intent: "draft a hello")

      expect(response.body).to include('data-controller="compose-intent"')
      expect(response.body).to include('value="draft a hello"')
    end

    it "prefills the recipient from an explicit ?to= address" do
      get new_email_message_path(to: "sofia@example.com")

      expect(to_field_value(response.body)).to eq("Sofia Martins <sofia@example.com>")
    end

    it "renders a blank intent composer with no params" do
      get new_email_message_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include('data-controller="compose-intent"')
      expect(to_field_value(response.body)).to eq("")
    end
  end

  context "in the classic layout" do
    before do
      allow(Features).to receive(:bold_layout?).and_return(true)
      user.update!(layout_mode: :classic)
    end

    it "ignores the intent params and renders the classic Desk" do
      get new_email_message_path(intent: "write to Sofia about the deck", to: "sofia@example.com")

      expect(response).to have_http_status(:ok)
      expect(to_field_value(response.body)).to eq("")
      expect(response.body).not_to include('data-controller="compose-intent"')
    end
  end

  context "with the flag off" do
    before { allow(Features).to receive(:bold_layout?).and_return(false) }

    it "ignores the intent even for a bold-preference user" do
      get new_email_message_path(intent: "write to Sofia about the deck")

      expect(to_field_value(response.body)).to eq("")
      expect(response.body).not_to include('data-controller="compose-intent"')
    end
  end
end
