require "rails_helper"

RSpec.describe Notifier do
  before do
    allow_any_instance_of(Notification).to receive(:broadcast_replace_to)
    allow_any_instance_of(Notification).to receive(:broadcast_remove_to)
    allow_any_instance_of(Notification).to receive(:broadcast_append_to)
  end

  let(:user) { create(:user) }
  let(:workspace) { user.workspace }

  describe ".documents_need_review (rolling card)" do
    it "keeps one live card reflecting the review count and auto-resolves at zero" do
      create(:document, :in_review, workspace: workspace)
      create(:document, :in_review, workspace: workspace)

      Notifier.documents_need_review(workspace)
      key = "document_review/#{workspace.id}"
      card = user.notifications.active.find_by(group_key: key)
      expect(card.title).to eq("2 documents need review")
      expect(card.count).to eq(2)
      expect(card).to be_priority_action_required

      # A third document needs review: same card updates in place, not a new row.
      create(:document, :in_review, workspace: workspace)
      Notifier.documents_need_review(workspace)
      expect(user.notifications.active.where(group_key: key).count).to eq(1)
      expect(card.reload.count).to eq(3)

      # Everything reviewed: the card auto-resolves.
      workspace.documents.needs_review.update_all(review_status: Document.review_statuses[:approved])
      Notifier.documents_need_review(workspace, bump: false)
      expect(user.notifications.active.where(group_key: key)).to be_empty
    end
  end

  describe ".document_failed / .document_recovered" do
    it "raises an action-required notification and resolves it on recovery" do
      doc = create(:document, :ai_failed, workspace: workspace)

      Notifier.document_failed(doc)
      n = user.notifications.active.find_by(notifiable: doc)
      expect(n).to be_present
      expect(n).to be_priority_action_required

      Notifier.document_recovered(doc)
      expect(user.notifications.active.where(notifiable: doc)).to be_empty
    end
  end

  describe ".reconciliation_ready" do
    let(:reconciliation) { create(:reconciliation, :ready, workspace:, created_by: user) }

    it "sends a notification to all workspace users" do
      Notifier.reconciliation_ready(reconciliation)
      n = user.notifications.active.find_by(group_key: "reconciliation_ready/#{reconciliation.id}")
      expect(n).to be_present
      expect(n.title).to be_present
      expect(n.category).to eq("reconciliation")
    end
  end

  describe ".reconciliation_parse_failed" do
    let(:reconciliation) { create(:reconciliation, :failed, workspace:, created_by: user) }

    it "sends an action_required notification to all workspace users" do
      Notifier.reconciliation_parse_failed(reconciliation)
      n = user.notifications.active.find_by(group_key: "reconciliation_parse_failed/#{reconciliation.id}")
      expect(n).to be_present
      expect(n).to be_priority_action_required
    end
  end

  describe ".reconciliation_export_ready" do
    let(:reconciliation) { create(:reconciliation, :ready, workspace:, created_by: user) }

    it "sends a notification to all workspace users" do
      Notifier.reconciliation_export_ready(reconciliation)
      n = user.notifications.active.find_by(group_key: "reconciliation_export/#{reconciliation.id}")
      expect(n).to be_present
      expect(n.title).to be_present
    end
  end
end
