# frozen_string_literal: true

require "rails_helper"

RSpec.describe Onboarding::TemplateApplier do
  let(:workspace) { create(:workspace) }

  describe "#apply!" do
    subject { described_class.new(workspace, template_key).apply! }

    context "with the freelancer template" do
      let(:template_key) { "freelancer" }

      it "creates tags from the template" do
        expect { subject }.to change { workspace.tags.reload.count }.by(3)
      end

      it "creates document types from the template" do
        expect { subject }.to change { workspace.document_types.reload.count }.by(4)
      end

      it "stores the template key in workspace settings" do
        subject
        expect(workspace.reload.setting("setup_template")).to eq("freelancer")
      end

      it "sets module_visibility in workspace settings" do
        subject
        expect(workspace.reload.settings["module_visibility"]).to be_a(Hash)
      end

      it "is idempotent (applying twice does not duplicate)" do
        described_class.new(workspace, template_key).apply!
        expect { subject }.not_to change { workspace.tags.reload.count }
        expect { subject }.not_to change { workspace.document_types.reload.count }
      end

      it "returns a hash with :tags and :document_types" do
        result = subject
        expect(result).to have_key(:tags)
        expect(result).to have_key(:document_types)
      end

      it "creates tags with the correct names" do
        subject
        names = workspace.tags.pluck(:name)
        expect(names).to include("clients", "invoices", "projects")
      end
    end

    context "with the just_exploring template" do
      let(:template_key) { "just_exploring" }

      it "creates no tags" do
        expect { subject }.not_to change { workspace.tags.reload.count }
      end

      it "creates no document types" do
        expect { subject }.not_to change { workspace.document_types.reload.count }
      end

      it "still stores the template key" do
        subject
        expect(workspace.reload.setting("setup_template")).to eq("just_exploring")
      end
    end

    context "switching templates is additive" do
      it "does not remove tags created by a previous template" do
        described_class.new(workspace, "freelancer").apply!
        freelancer_tags = workspace.tags.pluck(:name)

        described_class.new(workspace, "job_hunt").apply!

        workspace.reload
        freelancer_tags.each do |name|
          expect(workspace.tags.pluck(:name)).to include(name)
        end
      end

      it "does not remove document types created by a previous template" do
        described_class.new(workspace, "freelancer").apply!
        freelancer_types = workspace.document_types.pluck(:name)

        described_class.new(workspace, "personal_admin").apply!

        workspace.reload
        freelancer_types.each do |name|
          expect(workspace.document_types.pluck(:name)).to include(name)
        end
      end
    end

    context "with module_visibility" do
      it "does not overwrite keys the user has already set" do
        workspace.settings["module_visibility"] = { "contacts" => false }
        workspace.save!

        described_class.new(workspace, "freelancer").apply!
        # The user's override (contacts=false) wins over the template default (true).
        expect(workspace.reload.settings.dig("module_visibility", "contacts")).to be(false)
      end
    end

    context "with an unknown template key" do
      let(:template_key) { "unicorn" }

      it "raises UnknownTemplate" do
        expect { subject }.to raise_error(Onboarding::TemplateApplier::UnknownTemplate)
      end
    end
  end
end
