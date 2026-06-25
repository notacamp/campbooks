require "rails_helper"

RSpec.describe ContactsController, type: :controller do
  render_views

  let(:user) { create(:user) }
  let(:workspace) { user.workspace }

  before do
    session_record = create(:session, user: user)
    # Controller specs carry no signed cookie, so resume_session can't find the
    # session on its own — hand it the record directly. The rest of the real auth
    # chain (require_authentication → set_current_workspace) then runs and
    # populates Current.session / Current.workspace for the duration of the request.
    allow(controller).to receive(:find_session_by_cookie).and_return(session_record)
    # Isolate these specs from the onboarding gate; an otherwise-empty workspace
    # would redirect to onboarding before the action runs.
    allow(controller).to receive(:redirect_to_onboarding_if_incomplete)
    # The behaviour under test is the enqueue, not AI-provider resolution. Since
    # #24 stopped counting a bare ANTHROPIC_API_KEY and this workspace configures
    # no provider, make text AI available explicitly.
    allow(Ai::ProviderSetup).to receive(:available?).and_return(true)
  end

  describe "GET index" do
    it "returns contacts list grouped by person" do
      john = create(:person, name: "John Doe", workspace: workspace)
      create(:contact, email: "john@example.com", person: john, workspace: workspace)
      jane = create(:person, name: "Jane Doe", workspace: workspace)
      create(:contact, email: "jane@example.com", person: jane, workspace: workspace)

      get :index
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("John Doe")
      expect(response.body).to include("Jane Doe")
      expect(response.body).to include("john@example.com")
      expect(response.body).to include("jane@example.com")
    end

    it "renders empty state when no people" do
      get :index
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("No people found")
    end
  end

  describe "GET show" do
    it "shows contact profile" do
      contact = create(:contact, :analyzed, email: "john@example.com", workspace: workspace)

      get :show, params: { id: contact.id }
      expect(response).to have_http_status(:ok)
      # Compare against unescaped HTML: Faker names/summaries can contain
      # apostrophes/ampersands that ERB escapes (e.g. "O'Hara" -> "O&#39;Hara").
      expect(CGI.unescapeHTML(response.body)).to include(contact.display_name)
      expect(CGI.unescapeHTML(response.body)).to include(contact.context_summary)
    end

    it "shows the analyze button for unanalyzed contacts" do
      unanalyzed = create(:contact, email: "new@example.com", workspace: workspace)

      get :show, params: { id: unanalyzed.id }
      expect(response.body).to include("Analyze")
    end
  end

  describe "POST analyze" do
    it "enqueues ContactAnalysisJob with force: true" do
      contact = create(:contact, email: "analyze@example.com", workspace: workspace)

      expect {
        post :analyze, params: { id: contact.id }, format: :turbo_stream
      }.to have_enqueued_job(ContactAnalysisJob).with(contact.id, force: true, prompt: nil)

      expect(response).to have_http_status(:ok)
    end
  end

  describe "GET popover" do
    it "renders the contact hover card by id" do
      contact = create(:contact, email: "pop@example.com", workspace: workspace)

      get :popover, params: { id: contact.id }
      expect(response).to have_http_status(:ok)
    end

    it "returns not found for an unknown contact" do
      get :popover, params: { id: 0 }
      expect(response).to have_http_status(:not_found)
    end
  end
end
