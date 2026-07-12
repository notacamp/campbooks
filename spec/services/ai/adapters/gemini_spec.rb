# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::Adapters::Gemini do
  let(:adapter) { described_class.new(api_key: "gkey") }

  describe "#embed" do
    let(:vectors) { [ Array.new(1536) { 0.5 } ] }
    let(:response_body) do
      { "embeddings" => vectors.map { |v| { "values" => v } } }.to_json
    end

    def stub_batch_embed(model:, expected_dimensions: nil)
      url_pattern = /models\/#{Regexp.escape(model)}:batchEmbedContents/
      stub_request(:post, url_pattern)
        .with do |req|
          body = JSON.parse(req.body)
          requests = body["requests"]
          next false unless requests.is_a?(Array) && requests.any?
          if expected_dimensions
            requests.all? { |r| r["outputDimensionality"] == expected_dimensions }
          else
            requests.none? { |r| r.key?("outputDimensionality") }
          end
        end
        .to_return(status: 200, body: response_body,
                   headers: { "Content-Type" => "application/json" })
    end

    before { WebMock.disable_net_connect! }
    after  { WebMock.allow_net_connect! }

    it "POSTs to the native batchEmbedContents URL" do
      stub = stub_batch_embed(model: "gemini-embedding-001", expected_dimensions: nil)
      adapter.embed("hello", model: "gemini-embedding-001")
      expect(stub).to have_been_requested
    end

    it "sends outputDimensionality when dimensions is provided" do
      stub = stub_batch_embed(model: "gemini-embedding-001", expected_dimensions: 1536)
      adapter.embed("hello", model: "gemini-embedding-001", dimensions: 1536)
      expect(stub).to have_been_requested
    end

    it "omits outputDimensionality when dimensions is nil" do
      stub = stub_batch_embed(model: "gemini-embedding-001", expected_dimensions: nil)
      adapter.embed("hello", model: "gemini-embedding-001", dimensions: nil)
      expect(stub).to have_been_requested
    end

    it "parses response embeddings in order" do
      vectors_multi = [ Array.new(1536) { 0.1 }, Array.new(1536) { 0.9 } ]
      multi_body = { "embeddings" => vectors_multi.map { |v| { "values" => v } } }.to_json

      stub_request(:post, /batchEmbedContents/)
        .to_return(status: 200, body: multi_body,
                   headers: { "Content-Type" => "application/json" })

      result = adapter.embed(%w[text1 text2], model: "gemini-embedding-001")
      expect(result.length).to eq(2)
      expect(result[0].first).to be_within(0.001).of(0.1)
      expect(result[1].first).to be_within(0.001).of(0.9)
    end

    it "returns a single vector (not array) when input is a scalar" do
      stub_request(:post, /batchEmbedContents/)
        .to_return(status: 200, body: response_body,
                   headers: { "Content-Type" => "application/json" })

      result = adapter.embed("single", model: "gemini-embedding-001")
      expect(result).to be_an(Array)
      expect(result.first).to be_a(Numeric) # flat vector, not nested
    end

    it "returns an array of vectors when input is an array" do
      stub_request(:post, /batchEmbedContents/)
        .to_return(status: 200, body: response_body,
                   headers: { "Content-Type" => "application/json" })

      result = adapter.embed([ "text" ], model: "gemini-embedding-001")
      expect(result).to be_an(Array)
      expect(result.first).to be_an(Array) # nested
    end

    it "raises Faraday::Error and logs on HTTP error" do
      stub_request(:post, /batchEmbedContents/).to_return(status: 500, body: "error")

      expect(Rails.logger).to receive(:error).with(/Gemini adapter.*Embedding error/)
      expect { adapter.embed("text", model: "gemini-embedding-001") }.to raise_error(Faraday::Error)
    end
  end
end
