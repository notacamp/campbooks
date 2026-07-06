# frozen_string_literal: true

require "rails_helper"

RSpec.describe Onboarding::Templates do
  describe ".all" do
    it "returns a non-empty array" do
      expect(described_class.all).not_to be_empty
    end

    it "includes just_exploring as a template" do
      keys = described_class.all.map { |t| t[:key] }
      expect(keys).to include("just_exploring")
    end
  end

  describe ".keys" do
    it "returns all template keys" do
      expect(described_class.keys).to match_array(%w[
        freelancer small_business personal_admin job_hunt just_exploring
      ])
    end
  end

  describe ".find" do
    it "returns the matching template" do
      tpl = described_class.find("freelancer")
      expect(tpl).to be_a(Hash)
      expect(tpl[:key]).to eq("freelancer")
    end

    it "returns nil for an unknown key" do
      expect(described_class.find("nonexistent")).to be_nil
    end
  end

  describe "catalog integrity" do
    described_class::CATALOG.each do |template|
      context "template #{template[:key]}" do
        it "has a key, icon, tags array, document_types array, and module_visibility" do
          expect(template[:key]).to be_a(String).and match(/\A[a-z_]+\z/)
          expect(template[:icon]).to be_a(String).and be_present
          expect(template[:tags]).to be_an(Array)
          expect(template[:document_types]).to be_an(Array)
          expect(template[:module_visibility]).to be_a(Hash)
        end

        template[:tags].each do |tag|
          it "tag '#{tag[:name]}' has name, color, and prompt" do
            expect(tag[:name]).to be_present
            expect(tag[:color]).to match(/\A#[0-9a-f]{6}\z/i)
            expect(tag[:prompt]).to be_present
          end
        end

        template[:document_types].each do |dt|
          it "document type '#{dt[:name]}' has name, color, category, and prompt" do
            expect(dt[:name]).to be_present
            expect(dt[:color]).to match(/\A#[0-9a-f]{6}\z/i)
            expect(dt[:category]).to be_present
            expect(DocumentType::CATEGORIES).to include(dt[:category])
            expect(dt[:prompt]).to be_present
          end
        end

        template[:module_visibility].each do |key, value|
          it "module_visibility key '#{key}' is a boolean" do
            expect(value).to be(true).or be(false)
          end
        end
      end
    end
  end
end
