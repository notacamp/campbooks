# frozen_string_literal: true

module Emails
  # Rung 2 of the triage ladder: rank workspace tags for an email by embedding
  # similarity. Embeds the email as "subject -- snippet" and finds the nearest
  # pre-computed tag vectors (SearchTagEmbedding, built from "name -- prompt").
  #
  # Real-data calibration showed email↔tag cosine similarity tops out around 0.35
  # (a topic match, not a near-duplicate), so this rung does NOT auto-assign on an
  # absolute threshold. Instead it hands a #shortlist of the top candidates to a
  # cheap model (rung 3, Emails::LlmTagPicker) which makes the actual call — much
  # cheaper than the full Ai::EmailClassifier, and far more reliable than a raw
  # similarity cutoff.
  class EmbeddingClassifier
    # Retained for callers that still want an absolute-confidence read; see the
    # calibration note above for why it's a weak signal on its own.
    DEFAULT_THRESHOLD = 0.78

    Verdict = Data.define(:tag, :group_name, :similarity) do
      def confident?(threshold = DEFAULT_THRESHOLD) = similarity >= threshold
    end

    def initialize(email, embedder: EmbeddingService, top_k: 5)
      @email = email
      @embedder = embedder
      @top_k = top_k
    end

    # Single best (nearest) tag verdict, or nil.
    def call
      verdicts.first
    end

    # Similarity-ranked candidate tags (nearest first) for rung 3 to choose among.
    def shortlist(limit: @top_k)
      verdicts.first(limit)
    end

    # Pure ranking step (no I/O) so it can be unit-tested. `neighbors` is a list
    # of records responding to #tag and #neighbor_distance (the cosine distance
    # the `neighbor` gem attaches to query results).
    def self.verdicts_from(neighbors)
      Array(neighbors).sort_by(&:neighbor_distance).filter_map do |nb|
        next unless nb.tag
        Verdict.new(tag: nb.tag, group_name: nb.tag.group_name, similarity: 1.0 - nb.neighbor_distance)
      end
    end

    # Single best verdict (nearest tag), or nil.
    def self.best_verdict(neighbors)
      verdicts_from(neighbors).first
    end

    private

    def verdicts
      @verdicts ||= begin
        vector = @embedder.embed(text_for_embedding, workspace: workspace)
        if vector.nil? || (vector.respond_to?(:empty?) && vector.empty?)
          []
        else
          neighbors = SearchTagEmbedding
            .where(workspace: workspace)
            .nearest_neighbors(:embedding, vector, distance: "cosine")
            .includes(:tag)
            .first(@top_k)
          self.class.verdicts_from(neighbors)
        end
      end
    end

    def text_for_embedding
      [ @email.subject, snippet ].compact.reject(&:empty?).join(" -- ")
    end

    def snippet
      raw = @email.try(:summary).presence || @email.try(:body).to_s
      raw.to_s.gsub(/\s+/, " ").strip[0, 500]
    end

    def workspace
      @workspace ||= @email.try(:searchable_workspace) ||
        @email.try(:email_account)&.try(:workspace)
    end
  end
end
