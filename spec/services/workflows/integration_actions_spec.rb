require "rails_helper"

# Guards that the Drive/Notion workflow actions stay wired end-to-end through the
# registry → executor seam (a missing run method would only surface at run time).
RSpec.describe "Workflow Drive/Notion actions" do
  ACTIONS = %w[
    google_drive_create_folder
    google_drive_upload
    notion_create_page
    notion_create_database_item
  ].freeze

  ACTIONS.each do |key|
    describe key do
      let(:definition) { Workflows::ActionRegistry.definition(key) }

      it "is registered as a non-HTTP (run) action" do
        expect(definition).not_to be_nil
        expect(definition.http?).to be(false)
      end

      it "names an executor method that exists" do
        expect(Workflows::Executor.private_method_defined?(definition.run)).to be(true)
      end

      it "appears in the step picker catalog" do
        expect(Workflows::ActionRegistry.picker_cards.map { |c| c[:key] }).to include(key)
      end
    end
  end

  it "permits every action's config keys for strong params" do
    keys = Workflows::ActionRegistry.config_keys
    expect(keys).to include("folder_name", "notion_database_id", "notion_file_property", "notion_integration_id")
  end
end
