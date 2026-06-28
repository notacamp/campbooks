require "rails_helper"

RSpec.describe GoogleDrive::FolderResolver do
  let(:client) { instance_double(GoogleDrive::Client) }
  let(:document) { build_stubbed(:document, document_date: Date.new(2026, 6, 27)) }
  let(:config) { build_stubbed(:google_drive_config, folder_id: "base-folder", subfolder_pattern: pattern) }
  let(:pattern) { "flat" }

  before { allow(document).to receive(:entity_display_name).and_return("ACME Corp") }

  describe "#call" do
    context "flat" do
      it "returns the config folder_id directly" do
        expect(described_class.new(document, config, client).call).to eq("base-folder")
      end
    end

    context "year" do
      let(:pattern) { "year" }

      it "creates a year subfolder" do
        allow(client).to receive(:find_or_create_folder).with([ "2026" ], root_folder_id: "base-folder").and_return("yr-id")
        expect(described_class.new(document, config, client).call).to eq("yr-id")
      end
    end

    context "year_month" do
      let(:pattern) { "year_month" }

      it "creates year+month subfolders" do
        allow(client).to receive(:find_or_create_folder).with([ "2026", "06_June" ], root_folder_id: "base-folder").and_return("mo-id")
        expect(described_class.new(document, config, client).call).to eq("mo-id")
      end

      it "falls back to today when document_date is nil" do
        allow(document).to receive(:document_date).and_return(nil)
        today = Date.current
        allow(client).to receive(:find_or_create_folder)
          .with([ today.year.to_s, today.strftime("%m_%B") ], root_folder_id: "base-folder")
          .and_return("today")
        expect(described_class.new(document, config, client).call).to eq("today")
      end
    end

    context "entity" do
      let(:pattern) { "entity" }

      it "creates an entity subfolder" do
        allow(client).to receive(:find_or_create_folder).with([ "ACME Corp" ], root_folder_id: "base-folder").and_return("ent-id")
        expect(described_class.new(document, config, client).call).to eq("ent-id")
      end

      it "returns base folder when entity is blank" do
        allow(document).to receive(:entity_display_name).and_return(nil)
        expect(described_class.new(document, config, client).call).to eq("base-folder")
      end
    end
  end
end
