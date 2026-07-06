# frozen_string_literal: true

require "rails_helper"

RSpec.describe Onboarding::TemplateApplier do
  let(:workspace) { create(:workspace) }

  describe "#apply! (single template)" do
    subject { described_class.new(workspace, "freelancer").apply! }

    it "creates tags from the template" do
      expect { subject }.to change { workspace.tags.reload.count }.by(3)
    end

    it "creates document types from the template" do
      expect { subject }.to change { workspace.document_types.reload.count }.by(4)
    end

    it "stores the template keys array in workspace settings" do
      subject
      expect(workspace.reload.settings["setup_templates"]).to eq([ "freelancer" ])
    end

    it "sets module_visibility in workspace settings" do
      subject
      expect(workspace.reload.settings["module_visibility"]).to be_a(Hash)
    end

    it "is idempotent (applying twice does not duplicate)" do
      described_class.new(workspace, "freelancer").apply!
      expect { described_class.new(workspace, "freelancer").apply! }
        .not_to change { workspace.tags.reload.count }
      expect { described_class.new(workspace, "freelancer").apply! }
        .not_to change { workspace.document_types.reload.count }
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

  describe "#apply! (just_exploring)" do
    subject { described_class.new(workspace, "just_exploring").apply! }

    it "creates no tags" do
      expect { subject }.not_to change { workspace.tags.reload.count }
    end

    it "creates no document types" do
      expect { subject }.not_to change { workspace.document_types.reload.count }
    end

    it "still stores the template keys" do
      subject
      expect(workspace.reload.settings["setup_templates"]).to eq([ "just_exploring" ])
    end
  end

  describe "#apply! (multiple templates)" do
    it "creates the union of tags from both templates" do
      described_class.new(workspace, %w[freelancer job_hunt]).apply!
      names = workspace.reload.tags.pluck(:name)
      # freelancer tags
      expect(names).to include("clients", "invoices", "projects")
      # job_hunt tags
      expect(names).to include("applications", "interviews", "offers")
    end

    it "creates the union of document types from both templates" do
      described_class.new(workspace, %w[freelancer job_hunt]).apply!
      names = workspace.reload.document_types.pluck(:name)
      expect(names).to include("invoice", "contract", "proposal", "receipt")
      expect(names).to include("correspondence")
    end

    it "stores all keys in settings" do
      described_class.new(workspace, %w[freelancer job_hunt]).apply!
      expect(workspace.reload.settings["setup_templates"]).to contain_exactly("freelancer", "job_hunt")
    end

    it "does not duplicate shared document types" do
      # Both freelancer and small_business define a 'contract' type.
      # Applying both should create it exactly once.
      described_class.new(workspace, %w[freelancer small_business]).apply!
      contract_count = workspace.document_types.where(name: "contract").count
      expect(contract_count).to eq(1)
    end

    it "is idempotent when applied twice with the same set" do
      described_class.new(workspace, %w[freelancer job_hunt]).apply!
      count_before = workspace.tags.count

      described_class.new(workspace, %w[freelancer job_hunt]).apply!
      expect(workspace.tags.reload.count).to eq(count_before)
    end
  end

  describe "#apply! — module visibility (ANY-visible rule)" do
    it "marks a module visible if ANY selected template enables it" do
      # personal_admin hides organizations; freelancer shows it
      described_class.new(workspace, %w[personal_admin freelancer]).apply!
      expect(workspace.reload.settings.dig("module_visibility", "organizations")).to be(true)
    end

    it "marks a module hidden only when ALL selected templates hide it" do
      # personal_admin hides organizations AND activity; just_exploring shows both
      described_class.new(workspace, %w[personal_admin just_exploring]).apply!
      expect(workspace.reload.settings.dig("module_visibility", "organizations")).to be(true)
      expect(workspace.reload.settings.dig("module_visibility", "activity")).to be(true)
    end

    it "does not overwrite keys the user has already set" do
      workspace.settings["module_visibility"] = { "contacts" => false }
      workspace.save!

      described_class.new(workspace, "freelancer").apply!
      # The user's override (contacts=false) wins over the template default (true).
      expect(workspace.reload.settings.dig("module_visibility", "contacts")).to be(false)
    end
  end

  describe "switching templates is additive" do
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

  describe "error handling" do
    it "raises UnknownTemplate for a completely unknown key" do
      expect {
        described_class.new(workspace, "unicorn")
      }.to raise_error(Onboarding::TemplateApplier::UnknownTemplate)
    end

    it "raises UnknownTemplate when any key in an array is unknown" do
      expect {
        described_class.new(workspace, %w[freelancer unicorn])
      }.to raise_error(Onboarding::TemplateApplier::UnknownTemplate)
    end

    it "does not raise for an empty array (skip path)" do
      expect { described_class.new(workspace, []).apply! }.not_to raise_error
    end
  end
end
