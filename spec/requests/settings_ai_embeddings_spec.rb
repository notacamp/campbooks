# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Settings AI Embeddings", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  let(:default_key)  { "openai/text-embedding-3-small" }
  let(:mistral_key)  { "mistral/mistral-embed" }
  let(:gemini_key)   { "gemini/gemini-embedding-001" }

  describe "PATCH /settings/ai/embeddings" do
    it "redirects unauthenticated users to sign-in" do
      patch embeddings_settings_ai_path, params: { embedding_model: default_key }
      expect(response).to redirect_to(new_session_path)
    end

    context "when signed in" do
      before { sign_in(user) }

      context "happy path" do
        before do
          allow(EmbeddingService).to receive(:available_for?).and_return(true)
        end

        it "updates the workspace embedding_model" do
          patch embeddings_settings_ai_path, params: { embedding_model: mistral_key }
          expect(workspace.reload.embedding_model).to eq(mistral_key)
        end

        it "enqueues Search::WorkspaceReembedJob" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: mistral_key }
          }.to have_enqueued_job(Search::WorkspaceReembedJob).with(workspace)
        end

        it "redirects to settings_ai_path with a success flash" do
          patch embeddings_settings_ai_path, params: { embedding_model: mistral_key }
          expect(response).to redirect_to(settings_ai_path)
        end
      end

      context "unknown key" do
        it "does not change embedding_model" do
          original = workspace.embedding_model
          patch embeddings_settings_ai_path, params: { embedding_model: "bad/unknown-model" }
          expect(workspace.reload.embedding_model).to eq(original)
        end

        it "does not enqueue a reembed job" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: "bad/unknown-model" }
          }.not_to have_enqueued_job(Search::WorkspaceReembedJob)
        end

        it "redirects with an error" do
          patch embeddings_settings_ai_path, params: { embedding_model: "bad/unknown-model" }
          expect(response).to redirect_to(settings_ai_path)
        end
      end

      context "region-blocked (EU residency + US-only provider)" do
        before { workspace.update!(required_data_region: "EU") }

        it "does not change embedding_model" do
          original = workspace.embedding_model
          # openai is a US provider, blocked under EU residency
          patch embeddings_settings_ai_path, params: { embedding_model: "openai/text-embedding-3-large" }
          expect(workspace.reload.embedding_model).to eq(original)
        end

        it "does not enqueue a reembed job" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: "openai/text-embedding-3-large" }
          }.not_to have_enqueued_job(Search::WorkspaceReembedJob)
        end

        it "redirects with an error" do
          patch embeddings_settings_ai_path, params: { embedding_model: "openai/text-embedding-3-large" }
          expect(response).to redirect_to(settings_ai_path)
        end
      end

      context "provider unavailable (adapter not configured)" do
        before do
          allow(EmbeddingService).to receive(:available_for?).and_return(false)
        end

        it "does not change embedding_model" do
          original = workspace.embedding_model
          patch embeddings_settings_ai_path, params: { embedding_model: gemini_key }
          expect(workspace.reload.embedding_model).to eq(original)
        end

        it "does not enqueue a reembed job" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: gemini_key }
          }.not_to have_enqueued_job(Search::WorkspaceReembedJob)
        end

        it "redirects with an error" do
          patch embeddings_settings_ai_path, params: { embedding_model: gemini_key }
          expect(response).to redirect_to(settings_ai_path)
        end
      end

      context "same-key re-submit (already selected)" do
        before do
          allow(EmbeddingService).to receive(:available_for?).and_return(true)
          workspace.update!(embedding_model: mistral_key)
        end

        it "does not enqueue a reembed job" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: mistral_key }
          }.not_to have_enqueued_job(Search::WorkspaceReembedJob)
        end

        it "redirects with a notice" do
          patch embeddings_settings_ai_path, params: { embedding_model: mistral_key }
          expect(response).to redirect_to(settings_ai_path)
        end
      end

      context "re-submit of nil (default) workspace via default key" do
        before do
          allow(EmbeddingService).to receive(:available_for?).and_return(true)
          workspace.update!(embedding_model: nil)
        end

        it "does not enqueue a reembed job when submitting the default key" do
          expect {
            patch embeddings_settings_ai_path, params: { embedding_model: default_key }
          }.not_to have_enqueued_job(Search::WorkspaceReembedJob)
        end
      end
    end
  end

  describe "GET /settings/ai" do
    before { sign_in(user) }

    it "renders the embeddings section heading" do
      get settings_ai_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Embedding Model")
    end

    context "with a stale corpus" do
      before do
        allow(EmbeddingService).to receive(:available_for?).and_return(true)

        stale_scope  = double("stale_scope", count: 5)
        chunks_scope = double("chunks_scope", count: 10, stale_for: stale_scope)
        allow(SearchChunk).to receive(:where).with(workspace: workspace).and_return(chunks_scope)
      end

      it "shows the reindex progress copy" do
        get settings_ai_path
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Re-indexing in progress")
      end
    end
  end
end
