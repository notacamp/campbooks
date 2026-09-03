require "rails_helper"

# POST /email_messages/rewrite_draft — the bold composer's Shorter / Warmer
# footer buttons. JSON in / JSON out; guarded by the text AI provider.
RSpec.describe "EmailCompose draft rewrite", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  def rewrite(params)
    post rewrite_draft_email_messages_path, params: params, as: :json
  end

  context "with a text AI provider" do
    before do
      allow_any_instance_of(EmailComposeController).to receive(:ai_provider_available?).and_return(true)
    end

    it "returns the rewritten body" do
      allow_any_instance_of(Ai::DraftRewriter).to receive(:rewrite).and_return("<p>Short.</p>")

      rewrite(body: "<p>A long draft.</p>", tone: "shorter")

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["body"]).to eq("<p>Short.</p>")
    end

    it "passes the tone and the user's writing style to the rewriter" do
      allow(user).to receive(:writing_style_prompt).and_return("style-x")
      rewriter = instance_double(Ai::DraftRewriter, rewrite: "<p>ok</p>")
      allow(Ai::DraftRewriter).to receive(:new).and_return(rewriter)

      rewrite(body: "<p>hi</p>", tone: "warmer")

      expect(rewriter).to have_received(:rewrite).with("<p>hi</p>", tone: "warmer", style: anything)
    end

    it "rejects an unknown tone" do
      rewrite(body: "<p>x</p>", tone: "spicy")

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body["error"]).to eq("invalid_tone")
    end

    it "reports failure when the rewriter returns nil" do
      allow_any_instance_of(Ai::DraftRewriter).to receive(:rewrite).and_return(nil)

      rewrite(body: "<p>x</p>", tone: "firmer")

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body["error"]).to eq("rewrite_failed")
    end
  end

  context "without a text AI provider" do
    it "fails closed with a service-unavailable JSON error" do
      allow_any_instance_of(EmailComposeController).to receive(:ai_provider_available?).and_return(false)

      rewrite(body: "<p>x</p>", tone: "shorter")

      expect(response).to have_http_status(:service_unavailable)
      expect(response.parsed_body["error"]).to eq("ai_provider_unconfigured")
    end
  end
end
