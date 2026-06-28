require "rails_helper"

RSpec.describe "PipelineBoard", type: :request do
  let(:user) { create(:user) }
  let(:workspace) { user.workspace }
  let(:pipeline) { create(:pipeline, :with_stages, workspace: workspace) }
  let(:entry) { pipeline.stages.ordered.first }
  let(:done)  { pipeline.stages.ordered.last }

  before { sign_in(user) }

  describe "GET /pipelines/:id/board" do
    it "renders the board" do
      doc = create(:document, workspace: workspace)
      create(:pipeline_membership, pipeline: pipeline, item: doc, current_stage: entry)
      get board_pipeline_path(pipeline)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(entry.name)
    end

    it "hides items the user cannot access (email from an unreadable account)" do
      account = create(:email_account, workspace: workspace)
      email = create(:email_message, email_account: account, subject: "Secret deal")
      create(:pipeline_membership, pipeline: pipeline, item: email, current_stage: entry)

      get board_pipeline_path(pipeline)
      expect(response.body).not_to include("Secret deal")
    end
  end

  describe "POST /pipelines/:id/move" do
    let(:membership) { create(:pipeline_membership, pipeline: pipeline, item: create(:document, workspace: workspace), current_stage: entry) }

    it "moves a card to another stage" do
      post move_pipeline_path(pipeline), params: { membership_id: membership.id, to_stage_id: done.id }, as: :json
      expect(response).to have_http_status(:ok)
      expect(membership.reload.current_stage).to eq(done)
    end

    it "refuses to move a card out of a terminal stage" do
      membership.update!(current_stage: done)
      post move_pipeline_path(pipeline), params: { membership_id: membership.id, to_stage_id: entry.id }, as: :json
      expect(response).to have_http_status(:unprocessable_entity)
      expect(membership.reload.current_stage).to eq(done)
    end

    it "returns 404 when moving an email the user cannot access" do
      account = create(:email_account, workspace: workspace)
      email = create(:email_message, email_account: account)
      hidden = create(:pipeline_membership, pipeline: pipeline, item: email, current_stage: entry)

      post move_pipeline_path(pipeline), params: { membership_id: hidden.id, to_stage_id: done.id }, as: :json
      expect(response).to have_http_status(:not_found)
      expect(hidden.reload.current_stage).to eq(entry)
    end

    it "returns 404 for a pipeline in another workspace" do
      other = create(:pipeline, :with_stages, workspace: create(:workspace))
      post move_pipeline_path(other), params: { membership_id: 1, to_stage_id: 1 }, as: :json
      expect(response).to have_http_status(:not_found)
    end
  end
end
