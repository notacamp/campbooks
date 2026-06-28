require "rails_helper"

RSpec.describe Pipelineable, type: :model do
  let(:pipeline) { create(:pipeline, :with_stages) }
  let(:entry) { pipeline.stages.ordered.first }
  let(:document) { create(:document, workspace: pipeline.workspace) }

  describe "#assign_to_pipeline!" do
    it "places the item in the entry stage" do
      membership = document.assign_to_pipeline!(pipeline)
      expect(membership).to be_persisted
      expect(membership.current_stage).to eq(entry)
    end

    it "fires stage_entered as the item enters" do
      allow(Events).to receive(:publish)
      document.assign_to_pipeline!(pipeline)
      expect(Events).to have_received(:publish).with("pipeline.stage_entered", any_args)
    end

    it "is idempotent and keeps the item's current stage on re-assign" do
      membership = document.assign_to_pipeline!(pipeline)
      membership.move_to!(pipeline.stages.ordered.last)

      again = document.assign_to_pipeline!(pipeline)
      expect(again).to eq(membership)
      expect(again.current_stage).to eq(pipeline.stages.ordered.last)
    end
  end

  describe "#current_stage_for" do
    it "returns the current stage of the item in a pipeline" do
      document.assign_to_pipeline!(pipeline)
      expect(document.current_stage_for(pipeline)).to eq(entry)
    end
  end
end
