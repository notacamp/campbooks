# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/triage"

RSpec.describe Emails::Triage do
  def rules(category, confidence = 0.8)
    static(Struct.new(:category, :confidence).new(category, confidence))
  end

  # an embedding double whose #shortlist returns the given verdicts
  def embedding(verdicts)
    Struct.new(:list) { def shortlist(limit: nil) = list }.new(verdicts)
  end

  def verdict(confident, tag: :a_tag, similarity: 0.5)
    Struct.new(:c, :tag, :similarity) { def confident?(*) = c }.new(confident, tag, similarity)
  end

  # a picker double whose #call returns the given result (or nil)
  def picker(result)
    Struct.new(:r) { def call = r }.new(result)
  end

  def static(result)
    Struct.new(:result) { def call = result }.new(result)
  end

  def run(rules:, embedding: nil, picker: nil)
    described_class.new(:email, rules: rules, embedding: embedding, picker: picker).call
  end

  it "sends important / sensitive mail straight to the full LLM, never embeds it" do
    spy = Class.new do
      attr_reader :called
      def shortlist(limit: nil) = (@called = true) && []
    end.new
    decision = run(rules: rules(:important), embedding: spy)
    expect(decision.needs_llm?).to be(true)
    expect(decision.source).to eq(:llm)
    expect(spy.called).to be_falsey
  end

  it "trusts a near-duplicate embedding match without spending a model call" do
    picker_spy = Class.new do
      attr_reader :called
      def call = (@called = true) && nil
    end.new
    top = verdict(true, tag: :promos, similarity: 0.83)
    decision = run(rules: rules(:promotions), embedding: embedding([ top ]), picker: picker_spy)
    expect(decision.source).to eq(:embedding)
    expect(decision.tag).to eq(:promos)
    expect(picker_spy.called).to be_falsey
  end

  it "lets the cheap model pick from the shortlist when no match is confident" do
    list = [ verdict(false, tag: :promos, similarity: 0.31), verdict(false, tag: :news, similarity: 0.22) ]
    chosen = Struct.new(:tag).new(:news)
    decision = run(rules: rules(:notifications), embedding: embedding(list), picker: picker(chosen))
    expect(decision.source).to eq(:cheap_llm)
    expect(decision.tag).to eq(:news)
    expect(decision.needs_llm?).to be(false)
  end

  it "escalates to the full LLM when the cheap model abstains" do
    list = [ verdict(false, tag: :promos, similarity: 0.31) ]
    decision = run(rules: rules(:notifications), embedding: embedding(list), picker: picker(nil))
    expect(decision.needs_llm?).to be(true)
  end

  it "escalates when the embedding shortlist is empty" do
    expect(run(rules: rules(:promotions), embedding: embedding([])).needs_llm?).to be(true)
  end

  it "degrades to the full LLM if the embedding rung errors" do
    boom = Class.new { def shortlist(limit: nil) = raise("embed down") }.new
    expect(run(rules: rules(:promotions), embedding: boom).needs_llm?).to be(true)
  end

  it "treats a cheap-model error as an abstain and escalates" do
    list = [ verdict(false, tag: :promos, similarity: 0.31) ]
    boom = Class.new { def call = raise("picker down") }.new
    expect(run(rules: rules(:notifications), embedding: embedding(list), picker: boom).needs_llm?).to be(true)
  end

  it "always carries the coarse category through" do
    expect(run(rules: rules(:promotions, 0.65), embedding: embedding([])).category).to eq(:promotions)
  end
end
