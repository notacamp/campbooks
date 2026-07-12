# frozen_string_literal: true

require "rails_helper"

RSpec.describe DimensionedEmbeddings do
  let(:workspace) { create(:workspace) }

  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }
  let(:large_entry)   { Ai::EmbeddingModels.find("openai/text-embedding-3-large") }

  let(:vec_1536) { Array.new(1536) { rand } }
  let(:vec_1024) { Array.new(1024) { rand } }
  let(:vec_3072) { Array.new(3072) { rand } }

  # Build a minimal searchable so SearchChunk/SearchRecord FKs resolve.
  # EmailMessage does not have a direct workspace= setter — workspace is reached
  # via email_account, so we build through that association.
  let(:email_account) { create(:email_account, workspace: workspace) }
  let(:email_message) { create(:email_message, email_account: email_account) }

  # -----------------------------------------------------------------------
  # Helpers
  # -----------------------------------------------------------------------

  def build_chunk(attrs = {})
    SearchChunk.new({ workspace: workspace, searchable: email_message,
                      content: "hello", position: 0 }.merge(attrs))
  end

  def create_chunk(attrs = {})
    SearchChunk.create!({ workspace: workspace, searchable: email_message,
                          content: "hello", position: 0 }.merge(attrs))
  end

  # -----------------------------------------------------------------------
  # embedding_column_for
  # -----------------------------------------------------------------------
  describe ".embedding_column_for" do
    it "returns the right column for SearchChunk/embedding/1536" do
      expect(SearchChunk.embedding_column_for(:embedding, 1536)).to eq(:embedding)
    end

    it "returns the right column for SearchChunk/embedding/1024" do
      expect(SearchChunk.embedding_column_for(:embedding, 1024)).to eq(:embedding_1024)
    end

    it "returns the right column for SearchChunk/embedding/3072" do
      expect(SearchChunk.embedding_column_for(:embedding, 3072)).to eq(:embedding_3072)
    end

    it "raises ArgumentError for unknown kind" do
      expect { SearchChunk.embedding_column_for(:nonexistent, 1536) }.to raise_error(ArgumentError, /Unknown embedding kind/)
    end

    it "raises ArgumentError for unknown dimensions" do
      expect { SearchChunk.embedding_column_for(:embedding, 512) }.to raise_error(ArgumentError, /Unknown dimensions/)
    end

    it "returns the right columns for SearchRecord content_embedding/title_embedding" do
      expect(SearchRecord.embedding_column_for(:content_embedding, 1024)).to eq(:content_embedding_1024)
      expect(SearchRecord.embedding_column_for(:title_embedding, 3072)).to eq(:title_embedding_3072)
    end
  end

  # -----------------------------------------------------------------------
  # assign_embedding
  # -----------------------------------------------------------------------
  describe "#assign_embedding" do
    context "with the default entry (1536 dims)" do
      it "writes to :embedding and nils out the other dim columns" do
        chunk = build_chunk
        chunk.assign_embedding(:embedding, vec_1536, entry: default_entry)

        expect(chunk.embedding).to eq(vec_1536)
        expect(chunk.embedding_1024).to be_nil
        expect(chunk.embedding_3072).to be_nil
      end

      it "stamps embedding_model with the entry's model" do
        chunk = build_chunk
        chunk.assign_embedding(:embedding, vec_1536, entry: default_entry)
        expect(chunk.embedding_model).to eq(default_entry.model)
      end
    end

    context "with mistral entry (1024 dims)" do
      it "writes to :embedding_1024 and nils the 1536 and 3072 columns" do
        chunk = build_chunk(embedding: vec_1536)
        chunk.assign_embedding(:embedding, vec_1024, entry: mistral_entry)

        expect(chunk.embedding_1024).to eq(vec_1024)
        expect(chunk.embedding).to be_nil
        expect(chunk.embedding_3072).to be_nil
        expect(chunk.embedding_model).to eq(mistral_entry.model)
      end
    end

    context "with large entry (3072 dims)" do
      it "writes to :embedding_3072 and nils the others" do
        chunk = build_chunk
        chunk.assign_embedding(:embedding, vec_3072, entry: large_entry)

        expect(chunk.embedding_3072).to eq(vec_3072)
        expect(chunk.embedding).to be_nil
        expect(chunk.embedding_1024).to be_nil
        expect(chunk.embedding_model).to eq(large_entry.model)
      end
    end

    context "with SearchRecord's content_embedding kind" do
      it "writes to the correct column" do
        record = SearchRecord.new(workspace: workspace, searchable: email_message,
                                  searchable_type: "EmailMessage")
        record.assign_embedding(:content_embedding, vec_1024, entry: mistral_entry)
        expect(record.content_embedding_1024).to eq(vec_1024)
        expect(record.content_embedding).to be_nil
        expect(record.content_embedding_3072).to be_nil
        expect(record.embedding_model).to eq(mistral_entry.model)
      end
    end

    it "raises ArgumentError for unknown kind" do
      chunk = build_chunk
      expect { chunk.assign_embedding(:bogus, [], entry: default_entry) }
        .to raise_error(ArgumentError, /Unknown embedding kind/)
    end

    it "does not save the record" do
      chunk = build_chunk
      chunk.assign_embedding(:embedding, vec_1536, entry: default_entry)
      expect(chunk).to be_new_record
    end
  end

  # -----------------------------------------------------------------------
  # embedding_vector
  # -----------------------------------------------------------------------
  describe "#embedding_vector" do
    it "returns the vector from the appropriate column" do
      chunk = build_chunk(embedding_1024: vec_1024)
      expect(chunk.embedding_vector(:embedding, 1024)).to eq(vec_1024)
    end
  end

  # -----------------------------------------------------------------------
  # stamp_matches?
  # -----------------------------------------------------------------------
  describe "#stamp_matches?" do
    context "default entry" do
      it "returns true when embedding_model equals the entry's model" do
        chunk = build_chunk(embedding_model: default_entry.model)
        expect(chunk.stamp_matches?(default_entry)).to be(true)
      end

      it "returns true when embedding_model is nil (legacy row)" do
        chunk = build_chunk(embedding_model: nil)
        expect(chunk.stamp_matches?(default_entry)).to be(true)
      end

      it "returns false when embedding_model is a different model" do
        chunk = build_chunk(embedding_model: "mistral-embed")
        expect(chunk.stamp_matches?(default_entry)).to be(false)
      end
    end

    context "non-default entry (mistral)" do
      it "returns true when embedding_model equals the entry's model" do
        chunk = build_chunk(embedding_model: mistral_entry.model)
        expect(chunk.stamp_matches?(mistral_entry)).to be(true)
      end

      it "returns false when embedding_model is nil" do
        chunk = build_chunk(embedding_model: nil)
        expect(chunk.stamp_matches?(mistral_entry)).to be(false)
      end

      it "returns false when embedding_model is a different model" do
        chunk = build_chunk(embedding_model: default_entry.model)
        expect(chunk.stamp_matches?(mistral_entry)).to be(false)
      end
    end
  end

  # -----------------------------------------------------------------------
  # fresh_for / stale_for — full truth table
  # Each scenario: stamp=NULL, stamp=model, stamp=other × column present/null
  # × default vs non-default entry
  # -----------------------------------------------------------------------
  describe ".fresh_for and .stale_for" do
    # We exercise through SearchChunk (kind: :embedding, default kind).

    shared_examples "fresh/stale correctness" do |entry_name, stamp_desc, stamp_value, col_present, expect_fresh|
      context "#{entry_name} entry | stamp=#{stamp_desc} | column #{col_present ? 'present' : 'null'}" do
        let(:entry) do
          case entry_name
          when :default then default_entry
          when :mistral then mistral_entry
          end
        end

        let!(:chunk) do
          vec = col_present ? Array.new(entry.dimensions) { 0.1 } : nil
          col = SearchChunk.embedding_column_for(:embedding, entry.dimensions)
          create_chunk(embedding_model: stamp_value, col => vec)
        end

        it "fresh_for includes the row: #{expect_fresh}" do
          ids = SearchChunk.fresh_for(entry).pluck(:id)
          if expect_fresh
            expect(ids).to include(chunk.id)
          else
            expect(ids).not_to include(chunk.id)
          end
        end

        it "stale_for is the exact complement" do
          fresh_ids  = SearchChunk.fresh_for(entry).pluck(:id)
          stale_ids  = SearchChunk.stale_for(entry).pluck(:id)
          all_ids    = SearchChunk.pluck(:id)
          # Every row appears in exactly one of the two scopes.
          expect((fresh_ids + stale_ids).sort).to eq(all_ids.sort)
          expect((fresh_ids & stale_ids)).to be_empty
        end
      end
    end

    # Default entry (text-embedding-3-small)
    include_examples "fresh/stale correctness", :default, "NULL",    nil,                            true,  true
    include_examples "fresh/stale correctness", :default, "NULL",    nil,                            false, false
    include_examples "fresh/stale correctness", :default, "model",   "text-embedding-3-small",       true,  true
    include_examples "fresh/stale correctness", :default, "model",   "text-embedding-3-small",       false, false
    include_examples "fresh/stale correctness", :default, "other",   "mistral-embed",                true,  false
    include_examples "fresh/stale correctness", :default, "other",   "mistral-embed",                false, false

    # Non-default entry (mistral)
    include_examples "fresh/stale correctness", :mistral, "NULL",    nil,                            true,  false
    include_examples "fresh/stale correctness", :mistral, "NULL",    nil,                            false, false
    include_examples "fresh/stale correctness", :mistral, "model",   "mistral-embed",                true,  true
    include_examples "fresh/stale correctness", :mistral, "model",   "mistral-embed",                false, false
    include_examples "fresh/stale correctness", :mistral, "other",   "text-embedding-3-small",       true,  false
    include_examples "fresh/stale correctness", :mistral, "other",   "text-embedding-3-small",       false, false
  end
end
