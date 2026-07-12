# frozen_string_literal: true

require "rails_helper"

RSpec.describe SearchService do
  let(:workspace)     { create(:workspace) }
  let(:email_account) { create(:email_account, workspace: workspace) }
  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }

  def vec(dims)
    Array.new(dims) { 0.5 }
  end

  def create_record(searchable, entry: default_entry, content_vec: nil, title_vec: nil)
    content_col = SearchRecord.embedding_column_for(:content_embedding, entry.dimensions)
    title_col   = SearchRecord.embedding_column_for(:title_embedding, entry.dimensions)
    SearchRecord.create!(
      workspace:         workspace,
      searchable:        searchable,
      title:             searchable.try(:subject) || "Title",
      content_preview:   "preview",
      tags:              [],
      filter_data:       {},
      source_created_at: Time.current,
      indexed_at:        Time.current,
      embedding_model:   entry.model,
      content_col =>     content_vec || vec(entry.dimensions),
      title_col =>       title_vec
    )
  end

  # Stub SearchService#vector_search to return the provided records,
  # bypassing the nearest-neighbors HNSW call which is not available in tests.
  def stub_vector_search(service, records)
    allow(service).to receive(:vector_search).and_return(records)
  end

  # -----------------------------------------------------------------------
  # Default workspace — legacy unstamped records are still found
  # -----------------------------------------------------------------------
  describe "default workspace (OpenAI)" do
    let(:email_message) { create(:email_message, email_account: email_account) }

    it "requests the query embedding through the default entry" do
      record = create_record(email_message)
      svc = described_class.new(workspace)
      stub_vector_search(svc, [ record ])

      expect(EmbeddingService).to receive(:embed).with(
        "invoice",
        workspace: workspace,
        entry: default_entry
      ).and_return(vec(1536))

      svc.search("invoice")
    end

    it "includes legacy unstamped records (NULL stamp treated as fresh for default)" do
      email2 = create(:email_message, email_account: email_account)
      # Legacy row: NULL stamp, 1536 content_embedding
      legacy_record = SearchRecord.create!(
        workspace:         workspace,
        searchable:        email2,
        title:             "Legacy",
        content_preview:   "preview",
        tags:              [],
        filter_data:       {},
        source_created_at: Time.current,
        indexed_at:        Time.current,
        content_embedding: vec(1536),
        embedding_model:   nil
      )

      svc = described_class.new(workspace)
      stub_vector_search(svc, [ legacy_record ])

      allow(EmbeddingService).to receive(:embed).and_return(vec(1536))

      results = svc.search("hello")
      expect(results.map(&:search_record)).to include(legacy_record)
    end
  end

  # -----------------------------------------------------------------------
  # Mistral workspace — only mistral-stamped records visible
  # -----------------------------------------------------------------------
  describe "mistral workspace" do
    let(:email_message) { create(:email_message, email_account: email_account) }

    before { workspace.update!(embedding_model: "mistral/mistral-embed") }

    it "requests the query embedding through the mistral entry" do
      record = create_record(email_message, entry: mistral_entry)
      svc = described_class.new(workspace)
      stub_vector_search(svc, [ record ])

      expect(EmbeddingService).to receive(:embed).with(
        "invoice",
        workspace: workspace,
        entry: mistral_entry
      ).and_return(vec(1024))

      svc.search("invoice")
    end

    it "excludes old 1536 records (different embedding space)" do
      # Stale 1536 record for the same searchable — must not appear in results
      old_record = SearchRecord.create!(
        workspace:         workspace,
        searchable:        email_message,
        title:             "Old",
        content_preview:   "preview",
        tags:              [],
        filter_data:       {},
        source_created_at: Time.current,
        indexed_at:        Time.current,
        content_embedding: vec(1536),
        embedding_model:   default_entry.model
      )

      allow(EmbeddingService).to receive(:embed).and_return(vec(1024))

      # vector_search should not return the stale record; verify via the
      # fresh_for scope that drives the DB query.
      fresh_ids = SearchRecord.where(workspace_id: workspace.id)
                              .fresh_for(mistral_entry, kind: :content_embedding)
                              .pluck(:id)

      expect(fresh_ids).not_to include(old_record.id)
    end
  end

  # -----------------------------------------------------------------------
  # compute_scores reads vectors via embedding_vector (entry-aware)
  # -----------------------------------------------------------------------
  describe "#compute_scores" do
    let(:email_message) { create(:email_message, email_account: email_account) }

    it "reads content and title vectors from the entry-appropriate dim column" do
      record = create_record(email_message, entry: mistral_entry, content_vec: vec(1024), title_vec: vec(1024))
      workspace.update!(embedding_model: "mistral/mistral-embed")

      svc = described_class.new(workspace)
      entry = mistral_entry

      results = svc.send(:compute_scores, [ record ], vec(1024),
                         described_class::DEFAULT_OPTIONS, entry: entry)

      expect(results).not_to be_empty
      expect(results.first.content_similarity).to be_a(Float)
    end
  end
end
