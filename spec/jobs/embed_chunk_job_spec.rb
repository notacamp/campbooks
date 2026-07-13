# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmbedChunkJob, type: :job do
  let(:workspace)     { create(:workspace) }
  let(:email_account) { create(:email_account, workspace: workspace) }
  let(:email_message) { create(:email_message, email_account: email_account) }
  let(:default_entry) { Ai::EmbeddingModels::DEFAULT }
  let(:mistral_entry) { Ai::EmbeddingModels.find("mistral/mistral-embed") }

  def create_chunk(entry: default_entry, vec: nil, position: 0)
    col = SearchChunk.embedding_column_for(:embedding, entry.dimensions)
    attrs = {
      workspace:       workspace,
      searchable:      email_message,
      content:         "chunk content #{position}",
      position:        position,
      embedding_model: vec ? entry.model : nil
    }
    attrs[col] = vec
    SearchChunk.create!(attrs)
  end

  def vec(dims)
    Array.new(dims) { 0.5 }
  end

  # -----------------------------------------------------------------------
  # Transient provider errors → job retries rather than going straight to failed
  # -----------------------------------------------------------------------
  describe "retry on transient provider errors" do
    it "is configured to retry on every TRANSIENT_ERRORS class" do
      retry_handlers = described_class.rescue_handlers.map(&:first)
      Ai::Adapters::Base::TRANSIENT_ERRORS.each do |error_class|
        expect(retry_handlers).to include(error_class.to_s),
          "Expected EmbedChunkJob to retry on #{error_class}, but no retry_on handler found"
      end
    end

    it "re-raises a rate-limit error from #perform so retry_on can schedule the retry" do
      chunk = create_chunk(entry: default_entry)

      allow(EmbeddingService).to receive(:embed)
        .and_raise(Faraday::TooManyRequestsError.new(nil, nil))

      # Call #perform directly: perform_now would let retry_on intercept the
      # exception and schedule a retry instead of raising.
      expect {
        described_class.new.perform(chunk)
      }.to raise_error(Faraday::TooManyRequestsError)
    end
  end

  describe "mistral workspace" do
    before { workspace.update!(embedding_model: "mistral/mistral-embed") }

    it "writes the vector into embedding_1024, stamps 'mistral-embed', and clears embedding" do
      chunk = create_chunk(entry: mistral_entry)
      allow(EmbeddingService).to receive(:embed).and_return(vec(1024))

      described_class.perform_now(chunk)
      chunk.reload

      expect(chunk.embedding_model).to eq("mistral-embed")
      expect(chunk.embedding_1024).to be_present
      expect(chunk.embedding).to be_nil
    end
  end

  describe "skip condition: legacy NULL-stamp chunk with default workspace" do
    it "skips a chunk that already has a 1536 vector and NULL stamp (treated as fresh for default)" do
      # Legacy row: stamp is nil but the 1536 column is populated — stamp_matches? returns
      # true for the default entry and embedding_vector is present, so it must be skipped.
      chunk = SearchChunk.create!(
        workspace:       workspace,
        searchable:      email_message,
        content:         "old content",
        position:        0,
        embedding:       vec(1536),
        embedding_model: nil
      )

      expect(EmbeddingService).not_to receive(:embed)
      described_class.perform_now(chunk)
    end
  end

  describe "finalize enqueue when last chunk is done" do
    it "enqueues FinalizeSearchRecordJob once all chunks for the searchable are fresh" do
      c1 = create_chunk(position: 0, entry: default_entry, vec: vec(1536))
      c2 = create_chunk(position: 1)  # stale, no vector yet

      allow(EmbeddingService).to receive(:embed).and_return(vec(1536))

      expect {
        described_class.perform_now(c2)
      }.to have_enqueued_job(FinalizeSearchRecordJob).with(email_message.class.name, email_message.id)
    end

    it "does not enqueue FinalizeSearchRecordJob when other chunks are still stale" do
      c1 = create_chunk(position: 0)  # stale
      c2 = create_chunk(position: 1)  # will be embedded

      allow(EmbeddingService).to receive(:embed).and_return(vec(1536))

      expect {
        described_class.perform_now(c2)
      }.not_to have_enqueued_job(FinalizeSearchRecordJob)
    end
  end
end
