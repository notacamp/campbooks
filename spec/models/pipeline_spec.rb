require "rails_helper"

RSpec.describe Pipeline, type: :model do
  let(:workspace) { create(:workspace) }

  describe "validations" do
    it "requires a name" do
      expect(build(:pipeline, name: nil)).not_to be_valid
    end

    it "enforces case-insensitive name uniqueness per workspace" do
      create(:pipeline, workspace: workspace, name: "Invoices")
      dup = build(:pipeline, workspace: workspace, name: "invoices")
      expect(dup).not_to be_valid
    end

    it "allows the same name in a different workspace" do
      create(:pipeline, workspace: workspace, name: "Invoices")
      expect(build(:pipeline, workspace: create(:workspace), name: "Invoices")).to be_valid
    end
  end

  describe "applies_to" do
    it "scopes for_documents/for_emails by enum" do
      docs  = create(:pipeline, workspace: workspace, applies_to: :documents)
      mails = create(:pipeline, workspace: workspace, applies_to: :emails)
      both  = create(:pipeline, workspace: workspace, applies_to: :both)

      expect(Pipeline.for_documents).to include(docs, both)
      expect(Pipeline.for_documents).not_to include(mails)
      expect(Pipeline.for_emails).to include(mails, both)
      expect(Pipeline.for_emails).not_to include(docs)
    end
  end

  describe "#entry_stage" do
    it "is the lowest-position stage" do
      pipeline = create(:pipeline, workspace: workspace)
      create(:pipeline_stage, pipeline: pipeline, name: "B", position: 2)
      first = create(:pipeline_stage, pipeline: pipeline, name: "A", position: 1)
      expect(pipeline.entry_stage).to eq(first)
    end
  end

  describe "nested stages" do
    it "rejects stages with a blank name" do
      pipeline = create(:pipeline, workspace: workspace)
      pipeline.update(stages_attributes: [ { name: "" } ])
      expect(pipeline.stages).to be_empty
    end
  end

  describe "dependent destroy" do
    it "removes stages and memberships" do
      pipeline = create(:pipeline, :with_stages, workspace: workspace)
      create(:pipeline_membership, pipeline: pipeline, current_stage: pipeline.entry_stage)

      expect { pipeline.destroy }
        .to change(PipelineStage, :count).by(-2)
        .and change(PipelineMembership, :count).by(-1)
    end
  end
end
