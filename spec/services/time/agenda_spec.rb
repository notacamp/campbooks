require "rails_helper"

RSpec.describe Time::Agenda do
  include Rails.application.routes.url_helpers

  let(:user) { create(:user) }
  let(:workspace) { user.workspace }
  let(:window) { { from: Time.current.beginning_of_day, to: 7.days.from_now.end_of_day } }

  describe "deadlines Scout found in a document" do
    let!(:document) do
      create(:document, workspace: workspace).tap do |doc|
        doc.assign_title("Seguro Renovação 2026")
        doc.save!
      end
    end
    let!(:reminder) do
      create(:reminder, workspace: workspace, source: document, reminder_type: :renewal,
                        title: "Policy renewal", due_at: 2.days.from_now, all_day: true)
    end

    it "renders the deadline with the document as its source (regression: Document has no #title)" do
      items = described_class.for(user, **window)
      item = items.find(&:deadline?)

      expect(item).to be_present
      expect(item.title).to eq("Policy renewal")
      expect(item.source_label).to include("Seguro Renovação 2026")
      expect(item.source_path).to eq(document_path(document))
    end
  end
end
