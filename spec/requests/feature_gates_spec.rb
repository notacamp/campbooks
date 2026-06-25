require "rails_helper"

# Production-readiness gates (see Features). The suite defaults these ON
# (config/environments/test.rb) so the features' own specs keep exercising them;
# here we force each one OFF to prove the disabled-by-default behavior — hidden
# UI and 404 entry points — plus an ON sanity check that the gate is what differs.
RSpec.describe "Production-readiness feature gates", type: :request do
  describe "Workflows (ENABLE_WORKFLOWS)" do
    context "when disabled" do
      before { allow(Features).to receive(:workflows?).and_return(false) }

      it "404s the workflows UI" do
        get "/workflows"
        expect(response).to have_http_status(:not_found)
      end

      it "404s the public webhook ingress" do
        post "/webhooks/some-token"
        expect(response).to have_http_status(:not_found)
      end

      it "makes WorkflowTriggerJob inert — it never runs the executor" do
        expect(Workflows::Executor).not_to receive(:call)
        expect(WorkflowTriggerJob.new.perform(123)).to be_nil
      end
    end

    context "when enabled" do
      before { allow(Features).to receive(:workflows?).and_return(true) }

      it "reaches the auth gate (a redirect, not a 404)" do
        get "/workflows"
        expect(response).to have_http_status(:redirect)
      end
    end
  end

  describe "Inbox Board view (ENABLE_EMAIL_BOARD)" do
    context "when disabled" do
      before { allow(Features).to receive(:email_board?).and_return(false) }

      it "404s the board endpoint" do
        get "/email_messages/board"
        expect(response).to have_http_status(:not_found)
      end
    end
  end

  describe "Microsoft 365 (ENABLE_MICROSOFT)" do
    context "when disabled" do
      before { allow(Features).to receive(:microsoft?).and_return(false) }

      it "hides the Sign in with Microsoft button on the login page" do
        get new_session_path
        expect(response.body).not_to include("/session/microsoft")
      end

      it "404s the sign-in route" do
        get "/session/microsoft"
        expect(response).to have_http_status(:not_found)
      end

      it "404s the OAuth callback" do
        get "/oauth/microsoft/callback", params: { code: "x", state: "irrelevant" }
        expect(response).to have_http_status(:not_found)
      end
    end

    context "when enabled (the suite default)" do
      it "shows the Sign in with Microsoft button" do
        get new_session_path
        expect(response.body).to include("/session/microsoft")
      end
    end
  end
end
