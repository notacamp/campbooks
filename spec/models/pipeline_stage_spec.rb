require "rails_helper"

RSpec.describe PipelineStage, type: :model do
  let(:pipeline) { create(:pipeline) }

  describe "validations" do
    it "requires a name unique (case-insensitively) within the pipeline" do
      create(:pipeline_stage, pipeline: pipeline, name: "Review")
      expect(build(:pipeline_stage, pipeline: pipeline, name: "review")).not_to be_valid
    end

    it "requires an integer position" do
      expect(build(:pipeline_stage, position: nil)).not_to be_valid
      expect(build(:pipeline_stage, position: 1.5)).not_to be_valid
    end

    it "accepts only a 6-digit hex color (CSS-injection guard)" do
      expect(build(:pipeline_stage, color: "#6366f1")).to be_valid
      expect(build(:pipeline_stage, color: "#ABCDEF")).to be_valid
      expect(build(:pipeline_stage, color: "#fff")).not_to be_valid
      expect(build(:pipeline_stage, color: "red")).not_to be_valid
      expect(build(:pipeline_stage, color: "#000; background:url(x)")).not_to be_valid
    end
  end

  describe "scopes" do
    it "orders by position then id and partitions on terminal" do
      done = create(:pipeline_stage, pipeline: pipeline, position: 2, is_terminal: true)
      todo = create(:pipeline_stage, pipeline: pipeline, position: 1)

      expect(pipeline.stages.ordered).to eq([ todo, done ])
      expect(PipelineStage.terminal).to include(done)
      expect(PipelineStage.terminal).not_to include(todo)
      expect(PipelineStage.non_terminal).to include(todo)
    end
  end

  describe "dependent: :nullify" do
    it "nullifies a membership's current_stage when the stage is destroyed" do
      pipeline = create(:pipeline, :with_stages)
      stage = pipeline.entry_stage
      membership = create(:pipeline_membership, pipeline: pipeline, current_stage: stage)

      stage.destroy
      expect(membership.reload.current_stage_id).to be_nil
    end
  end
end
