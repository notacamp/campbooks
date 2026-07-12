# frozen_string_literal: true

require "rails_helper"

RSpec.describe Search::RecordFinalizer do
  let(:workspace)       { create(:workspace) }
  let(:email_account)   { create(:email_account, workspace: workspace) }
  let(:email_message)   { create(:email_message, email_account: email_account) }
  let(:default_entry)   { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry)   { Ai::EmbeddingModels.find("mistral/mistral-embed") }

  def vec(dims)
    Array.new(dims) { 0.1 }
  end

  def create_chunk(position:, token_count: 10, entry: mistral_entry, vec: nil)
    col = SearchChunk.embedding_column_for(:embedding, entry.dimensions)
    v   = vec || self.vec(entry.dimensions)
    SearchChunk.create!(
      workspace:       workspace,
      searchable:      email_message,
      content:         "chunk #{position}",
      position:        position,
      token_count:     token_count,
      embedding_model: entry.model,
      col => v
    )
  end

  describe ".call with mistral entry (1024 dims)" do
    before do
      allow(email_message).to receive(:searchable_title).and_return("Invoice")
      allow(email_message).to receive(:searchable_content_preview).and_return("preview")
      allow(email_message).to receive(:searchable_tags).and_return([])
      allow(email_message).to receive(:searchable_filter_data).and_return({})
    end

    it "writes the weighted-average vector into content_embedding_1024 and stamps 'mistral-embed'" do
      c1 = create_chunk(position: 0, token_count: 10)
      c2 = create_chunk(position: 1, token_count: 10)

      allow(EmbeddingService).to receive(:embed).and_return(vec(mistral_entry.dimensions))

      described_class.call(email_message, entry: mistral_entry, title_vector: vec(mistral_entry.dimensions))

      record = SearchRecord.find_by!(searchable: email_message)
      expect(record.embedding_model).to eq("mistral-embed")
      expect(record.content_embedding_1024).to be_present
      expect(record.content_embedding).to be_nil      # 1536 column cleared
      expect(record.content_embedding_3072).to be_nil  # 3072 column cleared
    end

    it "computes a weighted average proportional to token counts" do
      # Two chunks: weights 1/3 and 2/3 respectively.
      v1 = Array.new(1024, 0.0)
      v2 = Array.new(1024, 1.0)
      create_chunk(position: 0, token_count: 1, vec: v1)
      create_chunk(position: 1, token_count: 2, vec: v2)

      allow(EmbeddingService).to receive(:embed).and_return(vec(mistral_entry.dimensions))

      described_class.call(email_message, entry: mistral_entry, title_vector: vec(mistral_entry.dimensions))

      record = SearchRecord.find_by!(searchable: email_message)
      expected = Array.new(1024) { |_| (1.0 / 3.0) * 0.0 + (2.0 / 3.0) * 1.0 }
      expect(record.content_embedding_1024.map { |v| v.round(6) })
        .to eq(expected.map { |v| v.round(6) })
    end

    context "title_vector: :compute" do
      it "calls EmbeddingService.embed for the title and writes title_embedding_1024" do
        create_chunk(position: 0)
        title_vec = vec(mistral_entry.dimensions)
        allow(EmbeddingService).to receive(:embed).with("Invoice", workspace: workspace, entry: mistral_entry)
                                                  .and_return(title_vec)

        described_class.call(email_message, entry: mistral_entry)

        record = SearchRecord.find_by!(searchable: email_message)
        expect(record.title_embedding_1024).to eq(title_vec)
        expect(record.title_embedding).to be_nil       # 1536 column cleared
      end
    end

    context "title_vector: precomputed vector" do
      it "uses the given vector without calling EmbeddingService" do
        create_chunk(position: 0)
        title_vec = vec(mistral_entry.dimensions)

        expect(EmbeddingService).not_to receive(:embed)

        described_class.call(email_message, entry: mistral_entry, title_vector: title_vec)

        record = SearchRecord.find_by!(searchable: email_message)
        expect(record.title_embedding_1024).to eq(title_vec)
      end
    end

    context "nil title" do
      before { allow(email_message).to receive(:searchable_title).and_return(nil) }

      it "stores nil title_embedding without calling embed" do
        create_chunk(position: 0)

        expect(EmbeddingService).not_to receive(:embed)

        described_class.call(email_message, entry: mistral_entry, title_vector: nil)

        record = SearchRecord.find_by!(searchable: email_message)
        expect(record.title_embedding_1024).to be_nil
      end
    end

    context "empty chunks" do
      it "returns without creating a SearchRecord" do
        expect {
          described_class.call(email_message, entry: mistral_entry)
        }.not_to change(SearchRecord, :count)
      end
    end
  end
end
