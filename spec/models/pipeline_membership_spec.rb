require "rails_helper"

RSpec.describe PipelineMembership, type: :model do
  let(:pipeline) { create(:pipeline, :with_stages) }
  let(:entry) { pipeline.stages.ordered.first }
  let(:done)  { pipeline.stages.ordered.last }

  describe "polymorphic item" do
    it "links a Document with matching bigint ids" do
      doc = create(:document)
      membership = create(:pipeline_membership, pipeline: pipeline, item: doc)
      expect(membership.reload.item).to eq(doc)
    end

    it "only accepts Document or EmailMessage item types" do
      m = build(:pipeline_membership)
      m.item_type = "Workspace"
      expect(m).not_to be_valid
    end

    it "is unique per (pipeline, item)" do
      doc = create(:document)
      create(:pipeline_membership, pipeline: pipeline, item: doc)
      expect { create(:pipeline_membership, pipeline: pipeline, item: doc) }
        .to raise_error(ActiveRecord::RecordInvalid)
    end
  end

  describe "#move_to!" do
    subject(:membership) { create(:pipeline_membership, pipeline: pipeline, current_stage: entry) }

    it "moves to the new stage and stamps last_moved_at" do
      membership.update_column(:last_moved_at, 1.day.ago)
      membership.move_to!(done)
      expect(membership.reload.current_stage).to eq(done)
      expect(membership.last_moved_at).to be_within(5.seconds).of(Time.current)
    end

    it "fires pipeline.stage_entered with the stage payload" do
      expect(Events).to receive(:publish).with(
        "pipeline.stage_entered",
        hash_including(subject: membership.item, payload: hash_including(stage_id: done.id))
      )
      membership.move_to!(done)
    end

    it "is a no-op when the stage is unchanged" do
      expect(Events).not_to receive(:publish)
      expect { membership.move_to!(entry) }.not_to change { membership.reload.updated_at }
    end

    it "is a no-op when the stage is nil" do
      expect { membership.move_to!(nil) }.not_to(change { membership.reload.current_stage_id })
    end

    it "records the transition in stage_history, closing the previous entry" do
      membership.move_to!(done)
      history = membership.reload.stage_history
      expect(history.last).to include("stage_id" => done.id, "exited_at" => nil)
      expect(history.first["exited_at"]).to be_present
    end
  end
end
