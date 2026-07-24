# frozen_string_literal: true

module Search
  # Single batched, time-budgeted, self-re-enqueueing sweeper that migrates all
  # search data (chunks, records, tag embeddings) to a workspace's current
  # embedding model.
  #
  # IMPORTANT: this is the ONLY re-embed mechanism. Do NOT fan out per-item jobs
  # (a past per-item backfill buried the prod queue ~13k jobs deep and starved
  # all user-triggered work for hours). One sweeper per workspace keeps the
  # queue impact predictable and bounded.
  class WorkspaceReembedJob < ApplicationJob
    queue_as :default
    queue_with_priority BACKGROUND_PRIORITY

    # At most one active sweep per workspace; a second enqueue blocks until the
    # first finishes (or the lock expires after 10 minutes).
    limits_concurrency to: 1, key: ->(workspace) { "workspace_reembed:#{workspace.id}" },
                       duration: 10.minutes

    retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5)

    # Hard ceiling on a single run; leftover work is handled by re-enqueueing.
    TIME_BUDGET  = 4.minutes
    # Stays comfortably under Gemini batchEmbedContents' 100-request cap.
    BATCH_SIZE   = 50
    # Brief pause between provider calls to respect rate limits. Stubbed to 0 in specs.
    BATCH_PAUSE  = 0.2

    def perform(workspace)
      return unless workspace.ai_processing_enabled?
      return unless Ai::ProviderSetup.configured?(workspace, :embeddings)

      # Fresh read of the entry each run — a model switch mid-sweep causes this
      # job to converge on the latest choice rather than the old one.
      entry    = workspace.embedding_model_entry
      deadline = TIME_BUDGET.from_now

      chunks_done  = 0
      records_done = 0
      tags_done    = 0

      Current.set(workspace: workspace) do
        # ---------------------------------------------------------------
        # Phase 1 — re-embed stale search chunks
        # ---------------------------------------------------------------
        loop do
          batch = SearchChunk.where(workspace: workspace)
                             .stale_for(entry)
                             .order(:id)
                             .limit(BATCH_SIZE)
          break if batch.empty?

          # A blank-content chunk can never become fresh (nothing to embed), so
          # leaving it stale would reselect it forever and the sweep would never
          # terminate. It carries nothing searchable — destroy it. EmbeddingService
          # also silently drops blank inputs, which would misalign the
          # batch↔vectors zip below and attach vectors to the wrong chunks.
          embeddable, blank = batch.partition { |c| c.content.present? }
          blank.each do |chunk|
            Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                              "destroying blank-content SearchChunk #{chunk.id}")
            chunk.destroy!
          end
          next if embeddable.empty?

          vectors = EmbeddingService.embed_batch(embeddable.map(&:content), workspace: workspace, entry: entry)

          if vectors.blank?
            # The workspace passed the entry-gate but embed_batch returned nothing
            # mid-sweep — the provider became unavailable after the run started
            # (e.g. quota exhausted). Fail loudly so the job lands in the failed
            # set and is visible to operators, rather than vanishing silently with
            # work remaining.
            Rails.logger.error("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                               "embed_batch returned blank in phase 1 — provider unavailable mid-sweep")
            raise Ai::EmbeddingUnavailableError,
                  "embed_batch returned blank in phase 1 (workspace #{workspace.id})"
          end

          check_vectors!(vectors, embeddable.size, entry, workspace)

          embeddable.zip(vectors).each do |chunk, vec|
            chunk.assign_embedding(:embedding, vec, entry: entry)
            chunk.save!
            chunks_done += 1
          end

          if Time.current >= deadline
            Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                              "deadline hit after phase 1 — re-enqueueing")
            self.class.perform_later(workspace)
            return
          end

          sleep(BATCH_PAUSE)
        end

        # ---------------------------------------------------------------
        # Phase 2 — recompute stale search records
        #
        # Only process records whose searchable has no remaining stale chunks
        # AND at least one fresh chunk — records mid-re-embed are left for the
        # next pass (either a later sweep iteration or EmbedChunkJob's own
        # finalize trigger picks them up).
        # ---------------------------------------------------------------
        stale_chunk_col = SearchChunk.embedding_column_for(:embedding, entry.dimensions)

        stale_chunk_cond = if entry.default?
          ActiveRecord::Base.sanitize_sql_array(
            [ "(sc.embedding_model IS NOT NULL AND sc.embedding_model <> ?) " \
              "OR sc.#{stale_chunk_col} IS NULL",
              entry.model ]
          )
        else
          ActiveRecord::Base.sanitize_sql_array(
            [ "sc.embedding_model IS NULL OR sc.embedding_model <> ? " \
              "OR sc.#{stale_chunk_col} IS NULL",
              entry.model ]
          )
        end

        fresh_chunk_cond = if entry.default?
          ActiveRecord::Base.sanitize_sql_array(
            [ "(sc.embedding_model = ? OR sc.embedding_model IS NULL) " \
              "AND sc.#{stale_chunk_col} IS NOT NULL",
              entry.model ]
          )
        else
          ActiveRecord::Base.sanitize_sql_array(
            [ "sc.embedding_model = ? AND sc.#{stale_chunk_col} IS NOT NULL",
              entry.model ]
          )
        end

        loop do
          batch = SearchRecord.where(workspace_id: workspace.id)
                              .stale_for(entry, kind: :content_embedding)
                              .where(
                                "NOT EXISTS (" \
                                  "SELECT 1 FROM search_chunks sc " \
                                  "WHERE sc.searchable_type = search_records.searchable_type " \
                                    "AND sc.searchable_id = search_records.searchable_id " \
                                    "AND (#{stale_chunk_cond})" \
                                ")"
                              )
                              .where(
                                "EXISTS (" \
                                  "SELECT 1 FROM search_chunks sc " \
                                  "WHERE sc.searchable_type = search_records.searchable_type " \
                                    "AND sc.searchable_id = search_records.searchable_id " \
                                    "AND (#{fresh_chunk_cond})" \
                                ")"
                              )
                              .order(:id)
                              .limit(BATCH_SIZE)
          break if batch.empty?

          # Batch-embed all present titles in one API call so phase 2 costs
          # O(1) API round-trips per batch rather than one per record.
          title_texts   = batch.map { |sr| sr.title.presence }
          present_texts = title_texts.compact
          title_vectors = if present_texts.any?
            raw = EmbeddingService.embed_batch(present_texts, workspace: workspace, entry: entry)
            if raw.blank?
              Rails.logger.error("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                                 "embed_batch returned blank in phase 2 — provider unavailable mid-sweep")
              raise Ai::EmbeddingUnavailableError,
                    "embed_batch returned blank in phase 2 (workspace #{workspace.id})"
            end

            check_vectors!(raw, present_texts.size, entry, workspace)
            raw
          else
            []
          end

          # Map title vectors back to records by index among the non-blank titles.
          present_idx = 0
          title_map   = title_texts.map do |text|
            if text
              vec = title_vectors[present_idx]
              present_idx += 1
              vec
            end
          end

          batch.each_with_index do |sr, i|
            searchable = sr.searchable
            unless searchable
              # An orphan record (searchable deleted without callbacks) would be
              # reselected by this loop forever — it can never become fresh.
              # It has no purpose without its searchable; destroy it.
              Rails.logger.warn("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                                "destroying orphan SearchRecord #{sr.id} (#{sr.searchable_type}##{sr.searchable_id})")
              sr.destroy!
              next
            end

            Search::RecordFinalizer.call(searchable, entry: entry, title_vector: title_map[i])
            records_done += 1
          rescue ActiveRecord::RecordNotUnique
            # Concurrent finalize job won the upsert race — this record is now up to date.
            records_done += 1
          end

          if Time.current >= deadline
            Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                              "deadline hit after phase 2 — re-enqueueing")
            self.class.perform_later(workspace)
            return
          end

          sleep(BATCH_PAUSE)
        end

        # ---------------------------------------------------------------
        # Phase 3 — re-embed stale tag embeddings
        # ---------------------------------------------------------------
        loop do
          batch = SearchTagEmbedding.where(workspace: workspace)
                                   .stale_for(entry)
                                   .order(:id)
                                   .limit(BATCH_SIZE)
          break if batch.empty?

          # Resolve tags eagerly; destroy orphan rows (tag was deleted but the
          # embedding row escaped the cascade — it has no purpose without a tag).
          live_batch = batch.select do |row|
            if row.tag.nil?
              Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                                "destroying orphan SearchTagEmbedding #{row.id} (tag gone)")
              row.destroy!
              false
            else
              true
            end
          end

          next if live_batch.empty?

          contents = live_batch.map { |row| SearchTagEmbedding.embedding_text_for(row.tag) }
          vectors  = EmbeddingService.embed_batch(contents, workspace: workspace, entry: entry)

          if vectors.blank?
            Rails.logger.error("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                               "embed_batch returned blank in phase 3 — provider unavailable mid-sweep")
            raise Ai::EmbeddingUnavailableError,
                  "embed_batch returned blank in phase 3 (workspace #{workspace.id})"
          end

          check_vectors!(vectors, contents.size, entry, workspace)

          live_batch.each_with_index do |row, i|
            row.content_hash = Digest::SHA256.hexdigest(contents[i])
            row.assign_embedding(:embedding, vectors[i], entry: entry)
            row.save!
            tags_done += 1
          end

          if Time.current >= deadline
            Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} " \
                              "deadline hit after phase 3 — re-enqueueing")
            self.class.perform_later(workspace)
            return
          end

          sleep(BATCH_PAUSE)
        end
      end

      Rails.logger.info("[Search::WorkspaceReembedJob] workspace=#{workspace.id} sweep complete — " \
                        "chunks=#{chunks_done} records=#{records_done} tags=#{tags_done}")
    end

    private

    # A count mismatch means the batch↔vectors zip would attach vectors to the
    # wrong rows (silent corruption); a dimension mismatch would write into the
    # wrong column. Both are provider misbehaviour — write nothing and raise.
    def check_vectors!(vectors, expected_count, entry, workspace)
      unless vectors.size == expected_count
        raise "embed_batch returned #{vectors.size} vectors for #{expected_count} inputs " \
              "(#{entry.key}, workspace #{workspace.id})"
      end

      vectors.each do |vec|
        next if vec&.size == entry.dimensions
        raise "embed_batch returned #{vec&.size || "nil"}-dim vector; expected " \
              "#{entry.dimensions} for #{entry.key} (workspace #{workspace.id})"
      end
    end
  end
end
