# frozen_string_literal: true

require "rails_helper"

RSpec.describe Search::WorkspaceReembedJob, type: :job do
  # Prevent actual sleeps between batches during specs.
  before { stub_const("Search::WorkspaceReembedJob::BATCH_PAUSE", 0) }

  let(:workspace)     { create(:workspace) }
  let(:email_account) { create(:email_account, workspace: workspace) }
  let(:email_message) { create(:email_message, email_account: email_account) }
  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }

  def vec(dims)
    Array.new(dims) { 0.4 }
  end

  # -----------------------------------------------------------------------
  # Helpers to build search corpus
  # -----------------------------------------------------------------------

  def create_chunk(position: 0, entry: default_entry, token_count: 10)
    col = SearchChunk.embedding_column_for(:embedding, entry.dimensions)
    SearchChunk.create!(
      workspace:       workspace,
      searchable:      email_message,
      content:         "chunk #{position}",
      position:        position,
      token_count:     token_count,
      embedding_model: entry.model,
      col => vec(entry.dimensions)
    )
  end

  def create_stale_chunk(position: 0)
    # Default-stamped chunk (treated as stale for mistral)
    SearchChunk.create!(
      workspace:       workspace,
      searchable:      email_message,
      content:         "old content #{position}",
      position:        position,
      token_count:     10,
      embedding:       vec(1536),
      embedding_model: default_entry.model
    )
  end

  def create_stale_record(searchable = email_message)
    SearchRecord.create!(
      workspace:       workspace,
      searchable:      searchable,
      title:           searchable.try(:subject) || searchable.try(:name) || "Title",
      content_preview: "preview",
      tags:            [],
      filter_data:     {},
      content_embedding: vec(1536),
      embedding_model: default_entry.model,
      source_created_at: Time.current,
      indexed_at:      Time.current
    )
  end

  def create_stale_tag_embedding(tag)
    SearchTagEmbedding.create!(
      workspace:       workspace,
      tag:             tag,
      embedding:       vec(1536),
      embedding_model: default_entry.model,
      content_hash:    Digest::SHA256.hexdigest(SearchTagEmbedding.embedding_text_for(tag))
    )
  end

  # -----------------------------------------------------------------------
  # Unconfigured workspace → immediate return, no API calls
  # -----------------------------------------------------------------------
  describe "unconfigured workspace" do
    it "returns immediately without embedding when AI processing is disabled" do
      allow(workspace).to receive(:ai_processing_enabled?).and_return(false)
      expect(EmbeddingService).not_to receive(:embed_batch)
      described_class.perform_now(workspace)
    end

    it "returns immediately when embeddings not configured" do
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(false)
      expect(EmbeddingService).not_to receive(:embed_batch)
      described_class.perform_now(workspace)
    end
  end

  # -----------------------------------------------------------------------
  # Already-fresh corpus → no API calls, no re-enqueue
  # -----------------------------------------------------------------------
  describe "already-fresh corpus" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
      # Mistral chunks already fresh
      create_chunk(position: 0, entry: mistral_entry)
    end

    it "makes no embed_batch calls and does not re-enqueue" do
      expect(EmbeddingService).not_to receive(:embed_batch)
      expect {
        described_class.perform_now(workspace)
      }.not_to have_enqueued_job(described_class)
    end
  end

  # -----------------------------------------------------------------------
  # Full sweep: stale corpus → all three phases complete
  # -----------------------------------------------------------------------
  describe "full sweep on mistral workspace" do
    let(:tag) { create(:tag, workspace: workspace, name: "Finance") }

    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)

      allow(email_message).to receive(:searchable_title).and_return("Invoice")
      allow(email_message).to receive(:searchable_content_preview).and_return("preview")
      allow(email_message).to receive(:searchable_tags).and_return([])
      allow(email_message).to receive(:searchable_filter_data).and_return({})

      create_stale_chunk(position: 0)
      create_stale_record
      create_stale_tag_embedding(tag)

      allow(EmbeddingService).to receive(:embed_batch) do |_texts, **_opts|
        _texts.map { vec(1024) }
      end
      allow(EmbeddingService).to receive(:embed).and_return(vec(1024))
    end

    it "re-stamps chunks into embedding_1024 with mistral-embed" do
      described_class.perform_now(workspace)

      chunk = SearchChunk.where(workspace: workspace).first
      expect(chunk.embedding_model).to eq("mistral-embed")
      expect(chunk.embedding_1024).to be_present
      expect(chunk.embedding).to be_nil
    end

    it "recomputes the search record via RecordFinalizer (content_embedding_1024 set)" do
      described_class.perform_now(workspace)

      record = SearchRecord.find_by!(searchable: email_message)
      expect(record.embedding_model).to eq("mistral-embed")
      expect(record.content_embedding_1024).to be_present
      expect(record.content_embedding).to be_nil
    end

    it "re-embeds the tag into embedding_1024 with mistral-embed" do
      described_class.perform_now(workspace)

      ste = SearchTagEmbedding.find_by!(tag: tag)
      expect(ste.embedding_model).to eq("mistral-embed")
      expect(ste.embedding_1024).to be_present
      expect(ste.embedding).to be_nil
    end
  end

  # -----------------------------------------------------------------------
  # Phase-2 SQL filter: records with stale chunks are excluded
  #
  # Phase 1 runs before Phase 2, so to test the SQL filter in isolation we
  # query the phase-2 scope directly rather than running the full job.
  # -----------------------------------------------------------------------
  describe "Phase-2 correlated subquery excludes records with stale chunks" do
    it "does not include a record whose searchable still has a stale chunk" do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      entry = workspace.embedding_model_entry

      # Mixed state: one fresh mistral chunk + one stale default chunk
      create_chunk(position: 0, entry: mistral_entry)
      create_stale_chunk(position: 1)
      create_stale_record

      # Build the exact Phase-2 correlated scope from the job
      stale_col  = SearchChunk.embedding_column_for(:embedding, entry.dimensions)
      stale_cond = ActiveRecord::Base.sanitize_sql_array(
        [ "sc.embedding_model IS NULL OR sc.embedding_model <> ? OR sc.#{stale_col} IS NULL",
          entry.model ]
      )
      fresh_cond = ActiveRecord::Base.sanitize_sql_array(
        [ "sc.embedding_model = ? AND sc.#{stale_col} IS NOT NULL",
          entry.model ]
      )

      batch = SearchRecord.where(workspace_id: workspace.id)
                          .stale_for(entry, kind: :content_embedding)
                          .where(
                            "NOT EXISTS (SELECT 1 FROM search_chunks sc " \
                            "WHERE sc.searchable_type = search_records.searchable_type " \
                            "AND sc.searchable_id = search_records.searchable_id " \
                            "AND (#{stale_cond}))"
                          )
                          .where(
                            "EXISTS (SELECT 1 FROM search_chunks sc " \
                            "WHERE sc.searchable_type = search_records.searchable_type " \
                            "AND sc.searchable_id = search_records.searchable_id " \
                            "AND (#{fresh_cond}))"
                          )

      # The record is excluded because the stale chunk still exists
      expect(batch).to be_empty
    end
  end

  # -----------------------------------------------------------------------
  # Deadline hit → processes one batch then re-enqueues
  # -----------------------------------------------------------------------
  describe "deadline hit after first chunk batch" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)

      create_stale_chunk(position: 0)

      # Stub TIME_BUDGET to 0 so the deadline is immediately in the past
      stub_const("Search::WorkspaceReembedJob::TIME_BUDGET", 0.seconds)

      allow(EmbeddingService).to receive(:embed_batch) do |texts, **_opts|
        texts.map { vec(1024) }
      end
    end

    it "re-enqueues itself after processing the first batch" do
      expect {
        described_class.perform_now(workspace)
      }.to have_enqueued_job(described_class).with(workspace)
    end
  end

  # -----------------------------------------------------------------------
  # embed_batch returning [] → halts, does NOT re-enqueue
  # -----------------------------------------------------------------------
  describe "embed_batch returns blank" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
      create_stale_chunk(position: 0)
      allow(EmbeddingService).to receive(:embed_batch).and_return([])
    end

    it "halts and does NOT re-enqueue" do
      expect {
        described_class.perform_now(workspace)
      }.not_to have_enqueued_job(described_class)
    end
  end

  # -----------------------------------------------------------------------
  # Dimension mismatch → raises before writing
  # -----------------------------------------------------------------------
  describe "dimension mismatch" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
      create_stale_chunk(position: 0)
      # Adapter returns 5-dim vectors instead of 1024
      allow(EmbeddingService).to receive(:embed_batch).and_return([ Array.new(5, 0.1) ])
    end

    it "raises an error" do
      expect {
        described_class.perform_now(workspace)
      }.to raise_error(RuntimeError, /expected 1024/)
    end

    it "does not write any chunk before raising" do
      chunk = SearchChunk.where(workspace: workspace).first
      expect {
        described_class.perform_now(workspace) rescue nil
      }.not_to change { chunk.reload.embedding_1024 }.from(nil)
    end
  end

  # -----------------------------------------------------------------------
  # Orphan tag row → destroyed
  #
  # An orphan SearchTagEmbedding (tag association returns nil) can arise from
  # data corruption or a race condition. The job destroys such rows rather than
  # trying to re-embed them. We simulate the orphan state with a mock since
  # the DB-level FK prevents creating a real dangling row in tests.
  # -----------------------------------------------------------------------
  describe "orphan SearchTagEmbedding" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
    end

    it "destroys the orphan row and does not raise" do
      tag = create(:tag, workspace: workspace, name: "Orphan")
      ste = SearchTagEmbedding.create!(
        workspace:       workspace,
        tag:             tag,
        embedding:       vec(1536),
        embedding_model: default_entry.model,
        content_hash:    "abc"
      )
      orphan_id = ste.id

      # Simulate the orphan state: the tag association returns nil for all
      # SearchTagEmbedding instances in this run (mirrors the data-corruption
      # scenario the job guards against).
      allow_any_instance_of(SearchTagEmbedding).to receive(:tag).and_return(nil)

      described_class.perform_now(workspace)

      expect(SearchTagEmbedding.exists?(orphan_id)).to be(false)
    end
  end

  # -----------------------------------------------------------------------
  # Termination guards: rows that can never become fresh must be removed,
  # not reselected forever (a skip would loop the sweep indefinitely).
  # -----------------------------------------------------------------------
  describe "blank-content chunk" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
      allow(EmbeddingService).to receive(:embed_batch) { |texts, **| texts.map { vec(1024) } }
    end

    it "destroys it and still completes the sweep for the rest of the batch" do
      blank = create_stale_chunk(position: 0).tap { |c| c.update_columns(content: "   ") }
      real  = create_stale_chunk(position: 1)

      expect {
        described_class.perform_now(workspace)
      }.not_to have_enqueued_job(described_class)

      expect(SearchChunk.exists?(blank.id)).to be(false)
      expect(real.reload.embedding_model).to eq("mistral-embed")
      expect(real.embedding_1024).to be_present
    end
  end

  describe "orphan SearchRecord (searchable gone, fresh chunks present)" do
    before do
      workspace.update!(embedding_model: "mistral/mistral-embed")
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :embeddings).and_return(true)
      allow(EmbeddingService).to receive(:embed_batch) { |texts, **| texts.map { vec(1024) } }
    end

    it "destroys the orphan record instead of reselecting it forever" do
      create_chunk(position: 0, entry: mistral_entry) # fresh chunk so phase 2 selects the record
      record = create_stale_record
      orphan_id = record.id

      allow_any_instance_of(SearchRecord).to receive(:searchable).and_return(nil)

      expect {
        described_class.perform_now(workspace)
      }.not_to have_enqueued_job(described_class)

      expect(SearchRecord.exists?(orphan_id)).to be(false)
    end
  end
end
