require "rails_helper"

RSpec.describe "PipelineMemberships (item picker)", type: :request do
  let(:user) { create(:user) }
  let(:workspace) { user.workspace }
  let(:pipeline) { create(:pipeline, :with_stages, workspace: workspace) }
  let(:turbo) { { "Accept" => "text/vnd.turbo-stream.html" } }

  before { sign_in(user) }

  describe "GET /pipelines/:id/memberships/new (picker)" do
    it "lists addable documents and excludes ones already in the pipeline" do
      addable  = create(:document, workspace: workspace, metadata: { "title" => "Addable Doc" })
      assigned = create(:document, workspace: workspace, metadata: { "title" => "Assigned Doc" })
      assigned.assign_to_pipeline!(pipeline)

      get new_pipeline_membership_path(pipeline)
      expect(response.body).to include("Addable Doc")
      expect(response.body).not_to include("Assigned Doc")
      expect(addable).to be_present
    end

    it "excludes emails from accounts the user cannot read" do
      readable = create(:email_account, workspace: workspace)
      create(:email_account_user, :viewer, user: user, email_account: readable)
      create(:email_message, email_account: readable, subject: "Readable mail")

      hidden_account = create(:email_account, workspace: workspace)
      create(:email_message, email_account: hidden_account, subject: "Hidden mail")

      get new_pipeline_membership_path(pipeline)
      expect(response.body).to include("Readable mail")
      expect(response.body).not_to include("Hidden mail")
    end

    it "only offers documents when the pipeline applies_to documents" do
      pipeline.update!(applies_to: :documents)
      account = create(:email_account, workspace: workspace)
      create(:email_account_user, :viewer, user: user, email_account: account)
      create(:email_message, email_account: account, subject: "An email")

      get new_pipeline_membership_path(pipeline)
      expect(response.body).not_to include("An email")
    end
  end

  describe "POST /pipelines/:id/memberships" do
    it "adds the item at the entry stage" do
      doc = create(:document, workspace: workspace)
      expect {
        post pipeline_memberships_path(pipeline), params: { item_type: "Document", item_id: doc.id }, headers: turbo
      }.to change(pipeline.memberships, :count).by(1)
      expect(response).to have_http_status(:ok)
      expect(pipeline.memberships.last.current_stage).to eq(pipeline.entry_stage)
    end

    it "404s for an email the user cannot access" do
      account = create(:email_account, workspace: workspace)
      email = create(:email_message, email_account: account)
      expect {
        post pipeline_memberships_path(pipeline), params: { item_type: "EmailMessage", item_id: email.id }, headers: turbo
      }.not_to change(PipelineMembership, :count)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /pipelines/:id/memberships/:id" do
    it "removes the item from the pipeline" do
      doc = create(:document, workspace: workspace)
      membership = doc.assign_to_pipeline!(pipeline)
      expect {
        delete pipeline_membership_path(pipeline, membership), headers: turbo
      }.to change(PipelineMembership, :count).by(-1)
    end
  end
end
