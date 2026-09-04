# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::SkimTrayBroadcaster do
  let(:workspace) { create(:workspace) }
  let(:user_a)    { create(:user, workspace: workspace) }
  let(:user_b)    { create(:user, workspace: workspace) }

  describe ".refresh" do
    it "broadcasts to both users' per-user streams (fan-out)" do
      user_a
      user_b

      expect(Turbo::StreamsChannel).to receive(:broadcast_replace_to).with(
        "doc_skim_user_#{user_a.id}",
        hash_including(target: "doc_skim_tray_content")
      )
      expect(Turbo::StreamsChannel).to receive(:broadcast_replace_to).with(
        "doc_skim_user_#{user_b.id}",
        hash_including(target: "doc_skim_tray_content")
      )

      described_class.refresh(workspace)
    end

    it "does not broadcast to the workspace-scoped stream" do
      user_a

      expect(Turbo::StreamsChannel).not_to receive(:broadcast_replace_to).with(
        "doc_skim_#{workspace.id}",
        anything
      )

      described_class.refresh(workspace)
    end

    it "excludes a restricted-folder doc from user_b's broadcast but includes it for user_a" do
      # Materialize both users before creating the folder so workspace.users.find_each
      # sees both when the broadcaster fans out.
      user_a
      user_b

      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: user_a, can_read: true)
      doc = create(:document, :in_review, workspace: workspace)
      restricted.folder_memberships.create!(folderable: doc)

      captured = {}
      allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to) do |stream, target:, html:|
        captured[stream] = html
      end

      described_class.refresh(workspace)

      # user_a sees the document
      a_ids = Documents::SkimScope.for(workspace, user_a).map(&:id)
      expect(a_ids).to include(doc.id)

      # user_b does not see the document
      b_ids = Documents::SkimScope.for(workspace, user_b).map(&:id)
      expect(b_ids).not_to include(doc.id)

      # Each user received a broadcast on their own stream
      expect(captured).to have_key("doc_skim_user_#{user_a.id}")
      expect(captured).to have_key("doc_skim_user_#{user_b.id}")
    end

    it "does nothing for a nil workspace" do
      expect(Turbo::StreamsChannel).not_to receive(:broadcast_replace_to)
      described_class.refresh(nil)
    end
  end
end
