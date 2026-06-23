require "rails_helper"

RSpec.describe Workflows::ConditionEvaluator, type: :service do
  def step(config)
    WorkflowStep.new(step_type: "condition", config: config)
  end

  describe "document_type conditions (email context)" do
    let(:document) { double("Document", classification: double(name: "Invoice"), document_type: "expense_invoice") }
    let(:context) { instance_double(Workflows::EmailContext, documents: [ document ], liquid_context: {}) }

    it "matches on equals (case-insensitive)" do
      result = described_class.evaluate(step(field: "document_type", operator: "equals", value: "invoice"), context)
      expect(result).to be(true)
    end

    it "fails equals when no document matches" do
      result = described_class.evaluate(step(field: "document_type", operator: "equals", value: "receipt"), context)
      expect(result).to be(false)
    end

    it "supports not_equals" do
      result = described_class.evaluate(step(field: "document_type", operator: "not_equals", value: "receipt"), context)
      expect(result).to be(true)
    end

    it "supports contains" do
      result = described_class.evaluate(step(field: "document_type", operator: "contains", value: "invo"), context)
      expect(result).to be(true)
    end

    it "passes when the value is blank" do
      result = described_class.evaluate(step(field: "document_type", operator: "equals", value: ""), context)
      expect(result).to be(true)
    end
  end

  describe "expression conditions (webhook context)" do
    let(:context) { Workflows::WebhookContext.new(payload: { "status" => "paid", "amount" => "100" }) }

    it "evaluates a payload path with equals" do
      expect(described_class.evaluate(step(field: "payload.status", operator: "equals", value: "paid"), context)).to be(true)
      expect(described_class.evaluate(step(field: "payload.status", operator: "equals", value: "void"), context)).to be(false)
    end

    it "accepts a full Liquid expression as the field" do
      expect(described_class.evaluate(step(field: "{{ payload.status }}", operator: "equals", value: "paid"), context)).to be(true)
    end

    it "supports contains / not_contains" do
      expect(described_class.evaluate(step(field: "payload.status", operator: "contains", value: "pai"), context)).to be(true)
      expect(described_class.evaluate(step(field: "payload.status", operator: "not_contains", value: "void"), context)).to be(true)
    end

    it "supports exists / not_exists" do
      expect(described_class.evaluate(step(field: "payload.status", operator: "exists", value: ""), context)).to be(true)
      expect(described_class.evaluate(step(field: "payload.missing", operator: "not_exists", value: ""), context)).to be(true)
      expect(described_class.evaluate(step(field: "payload.missing", operator: "exists", value: ""), context)).to be(false)
    end

    it "passes an empty field (no condition configured)" do
      expect(described_class.evaluate(step(field: "", operator: "equals", value: "x"), context)).to be(true)
    end
  end
end
