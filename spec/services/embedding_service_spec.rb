# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmbeddingService do
  let(:ws) { create(:workspace) }

  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }
  let(:gemini_entry)  { Ai::EmbeddingModels.find("gemini/gemini-embedding-001") }
  let(:large_entry)   { Ai::EmbeddingModels.find("openai/text-embedding-3-large") }

  # Stub a successful adapter response (returns unit vectors of correct dims).
  def stub_adapter_embed(adapter, vectors)
    allow(adapter).to receive(:embed).and_return(vectors)
  end

  def unit_vector(dims)
    raw = Array.new(dims) { 1.0 / Math.sqrt(dims) }
    raw
  end

  # -----------------------------------------------------------------------
  # env_fallback_adapter (old tests reworked)
  # -----------------------------------------------------------------------
  describe "env-key fallback" do
    it "is disabled on the managed cloud (no silent platform-key embedding)" do
      with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
        svc = described_class.new(ws, entry: default_entry)
        expect(svc.send(:env_fallback_adapter)).to be_nil
      end
    end

    it "uses the operator's own key on self-hosted (same provider as entry)" do
      with_self_hosted do
        with_env("OPENAI_API_KEY" => "sk-test") do
          svc = described_class.new(ws, entry: default_entry)
          adapter = svc.send(:env_fallback_adapter)
          expect(adapter).to be_an_instance_of(Ai::Adapters::Openai)
        end
      end
    end

    it "returns nil on self-hosted without a matching provider key" do
      with_self_hosted do
        with_env("OPENAI_API_KEY" => nil, "GEMINI_API_KEY" => nil, "MISTRAL_API_KEY" => nil) do
          svc = described_class.new(ws, entry: default_entry)
          expect(svc.send(:env_fallback_adapter)).to be_nil
        end
      end
    end

    it "uses MISTRAL_API_KEY for the mistral entry on self-hosted" do
      with_self_hosted do
        with_env("MISTRAL_API_KEY" => "mkey") do
          svc = described_class.new(ws, entry: mistral_entry)
          adapter = svc.send(:env_fallback_adapter)
          expect(adapter).to be_an_instance_of(Ai::Adapters::Mistral)
        end
      end
    end

    it "does NOT fall back to a different provider's key (cross-provider fallback removed)" do
      with_self_hosted do
        # openai entry but only GEMINI_API_KEY present => no fallback
        with_env("OPENAI_API_KEY" => nil, "GEMINI_API_KEY" => "gkey") do
          svc = described_class.new(ws, entry: default_entry)
          expect(svc.send(:env_fallback_adapter)).to be_nil
        end
      end
    end
  end

  # -----------------------------------------------------------------------
  # embed_batch fails closed on the cloud
  # -----------------------------------------------------------------------
  it "embed_batch fails closed (nil) on the cloud with no configured adapter" do
    with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
      expect(described_class.new(ws).embed_batch([ "hello" ])).to be_nil
    end
  end

  # -----------------------------------------------------------------------
  # AI kill switch
  # -----------------------------------------------------------------------
  it "returns [] when ai_processing_enabled? is false" do
    allow(ws).to receive(:ai_processing_enabled?).and_return(false)
    expect(described_class.new(ws, entry: default_entry).embed_batch([ "hi" ])).to eq([])
  end

  # -----------------------------------------------------------------------
  # blank / empty input
  # -----------------------------------------------------------------------
  it "returns nil for blank text" do
    expect(described_class.new(ws, entry: default_entry).embed("")).to be_nil
  end

  it "returns [] for empty array" do
    expect(described_class.new(ws, entry: default_entry).embed_batch([])).to eq([])
  end

  # -----------------------------------------------------------------------
  # Region gating
  # -----------------------------------------------------------------------
  it "returns nil for EU workspace + openai entry (region blocked)" do
    allow(ws).to receive(:region_allows?).with("openai").and_return(false)
    expect(described_class.new(ws, entry: default_entry).embed_batch([ "text" ])).to be_nil
  end

  it "finds an adapter for EU workspace + mistral entry (Mistral is EU)" do
    allow(ws).to receive(:ai_processing_enabled?).and_return(true)
    allow(ws).to receive(:region_allows?).with("mistral").and_return(true)

    with_self_hosted do
      with_env("MISTRAL_API_KEY" => "mkey") do
        svc = described_class.new(ws, entry: mistral_entry)
        adapter = svc.send(:find_embedding_adapter)
        expect(adapter).to be_an_instance_of(Ai::Adapters::Mistral)
      end
    end
  end

  # -----------------------------------------------------------------------
  # Provider-directed resolution (BYO preferred over managed)
  # -----------------------------------------------------------------------
  it "prefers BYO (non-managed) adapter over managed for the entry's provider" do
    # No :ai_adapter factory; create directly through the workspace association.
    # managed adapter has managed: true (no stored api_key)
    ws.ai_adapters.create!(name: "managed-openai", provider: "openai",
                           managed: true, enabled: true)
    # BYO adapter has a stored key
    ws.ai_adapters.create!(name: "byo-openai", provider: "openai",
                           api_key: "sk-byo", enabled: true)

    svc = described_class.new(ws, entry: default_entry)
    adapter = svc.send(:find_embedding_adapter)

    # The BYO adapter wins: its api_key should be "sk-byo", not the platform env key
    expect(adapter).to be_an_instance_of(Ai::Adapters::Openai)
    expect(adapter.instance_variable_get(:@api_key)).to eq("sk-byo")
  end

  # -----------------------------------------------------------------------
  # correct URL / body per provider (WebMock)
  # -----------------------------------------------------------------------
  describe "WebMock: correct API endpoint and body" do
    before { WebMock.disable_net_connect! }
    after  { WebMock.allow_net_connect! }

    def make_openai_stub(url:, model:, dimensions: nil, dims: 1536)
      body_matcher = ->(b) {
        parsed = JSON.parse(b)
        parsed["model"] == model &&
          (dimensions.nil? ? !parsed.key?("dimensions") : parsed["dimensions"] == dimensions)
      }
      stub_request(:post, url)
        .with { |req| body_matcher.call(req.body) }
        .to_return(
          status: 200,
          body: { "data" => [ { "index" => 0, "embedding" => Array.new(dims) { 0.5 } } ] }.to_json,
          headers: { "Content-Type" => "application/json" }
        )
    end

    context "openai/text-embedding-3-small (default, no dimensions param)" do
      it "POSTs to openai embeddings URL without dimensions key" do
        stub = make_openai_stub(url: "https://api.openai.com/v1/embeddings",
                                model: "text-embedding-3-small", dimensions: nil, dims: 1536)
        with_self_hosted do
          with_env("OPENAI_API_KEY" => "sk-test") do
            result = described_class.embed("hello", workspace: ws, entry: default_entry)
            expect(stub).to have_been_requested
            expect(result).not_to be_nil
            expect(result.length).to eq(1536)
          end
        end
      end
    end

    context "mistral/mistral-embed (no dimensions param)" do
      it "POSTs to Mistral embeddings URL without dimensions key" do
        stub = make_openai_stub(url: "https://api.mistral.ai/v1/embeddings",
                                model: "mistral-embed", dimensions: nil, dims: 1024)
        with_self_hosted do
          with_env("MISTRAL_API_KEY" => "mk-test") do
            result = described_class.embed("hello", workspace: ws, entry: mistral_entry)
            expect(stub).to have_been_requested
            expect(result).not_to be_nil
          end
        end
      end
    end

    context "gemini/gemini-embedding-001 (outputDimensionality present)" do
      it "POSTs to Gemini batchEmbedContents URL with outputDimensionality" do
        stub = stub_request(:post, /batchEmbedContents/)
          .to_return(
            status: 200,
            body: { "embeddings" => [ { "values" => Array.new(1536) { 0.5 } } ] }.to_json,
            headers: { "Content-Type" => "application/json" }
          )

        with_self_hosted do
          with_env("GEMINI_API_KEY" => "gkey") do
            result = described_class.embed("hello", workspace: ws, entry: gemini_entry)
            expect(stub).to have_been_requested
            expect(result).not_to be_nil
          end
        end
      end
    end
  end

  # -----------------------------------------------------------------------
  # Normalization
  # -----------------------------------------------------------------------
  describe "L2 normalization" do
    it "normalizes non-unit vectors returned by the adapter" do
      non_unit = [ 3.0, 4.0 ]  # magnitude = 5, expected normalized: [0.6, 0.8]

      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:embed).and_return([ non_unit ])

      svc = described_class.new(ws, entry: default_entry)
      allow(svc).to receive(:find_embedding_adapter).and_return(adapter)

      result = svc.embed_batch([ "text" ])
      expect(result.first.map { |v| v.round(4) }).to eq([ 0.6, 0.8 ])
    end

    it "leaves zero vectors unchanged" do
      zero_vec = [ 0.0, 0.0, 0.0 ]

      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:embed).and_return([ zero_vec ])

      svc = described_class.new(ws, entry: default_entry)
      allow(svc).to receive(:find_embedding_adapter).and_return(adapter)

      result = svc.embed_batch([ "text" ])
      expect(result.first).to eq(zero_vec)
    end
  end

  # -----------------------------------------------------------------------
  # Truncation
  # -----------------------------------------------------------------------
  describe "input truncation" do
    it "truncates each text to entry.max_input_chars before calling the adapter" do
      adapter = instance_double(Ai::Adapters::Openai)
      captured = nil
      allow(adapter).to receive(:embed) { |texts, **_| captured = texts; [ [ 0.1 ] ] }

      svc = described_class.new(ws, entry: default_entry)
      allow(svc).to receive(:find_embedding_adapter).and_return(adapter)

      long_text = "x" * (default_entry.max_input_chars + 5000)
      svc.embed_batch([ long_text ])

      expect(captured.first.length).to eq(default_entry.max_input_chars)
    end

    it "does not truncate text within the limit" do
      adapter = instance_double(Ai::Adapters::Openai)
      captured = nil
      allow(adapter).to receive(:embed) { |texts, **_| captured = texts; [ [ 0.1 ] ] }

      svc = described_class.new(ws, entry: default_entry)
      allow(svc).to receive(:find_embedding_adapter).and_return(adapter)

      short_text = "hello world"
      svc.embed_batch([ short_text ])
      expect(captured.first).to eq(short_text)
    end
  end

  # -----------------------------------------------------------------------
  # Cross-provider env fallback REMOVED
  # -----------------------------------------------------------------------
  it "returns nil when the openai entry is used but only GEMINI_API_KEY is set (no cross-provider fallback)" do
    with_self_hosted do
      with_env("OPENAI_API_KEY" => nil, "GEMINI_API_KEY" => "gkey") do
        result = described_class.new(ws, entry: default_entry).embed_batch([ "text" ])
        expect(result).to be_nil
      end
    end
  end

  # -----------------------------------------------------------------------
  # available_for?
  # -----------------------------------------------------------------------
  describe ".available_for?" do
    it "returns false when no adapter can be resolved" do
      with_env("OPENAI_API_KEY" => nil) do
        expect(described_class.available_for?(ws, entry: default_entry)).to be(false)
      end
    end

    it "returns true when a self-hosted env key is present" do
      with_self_hosted do
        with_env("OPENAI_API_KEY" => "sk-test") do
          expect(described_class.available_for?(ws, entry: default_entry)).to be(true)
        end
      end
    end
  end
end
