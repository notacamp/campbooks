# frozen_string_literal: true

module Emails
  # Groups emails into Skim-Mode "stacks" by embedding similarity — the same
  # cheap vectors used for classification (rung 2) also collapse the noise:
  # e.g. 217 notifications → ~8 cards ("47 CircleCI builds", "31 GitHub PRs"),
  # with no LLM call.
  #
  # Greedy single-pass agglomeration: each item joins the most-similar existing
  # cluster above #min_similarity, else seeds a new one. O(n * clusters), fine
  # for the few hundred emails in a folder/group. Order-sensitive but
  # deterministic for a given input order. Pure / dependency-free for testing.
  class Clusterer
    DEFAULT_MIN_SIMILARITY = 0.82

    Cluster = Data.define(:member_ids, :centroid) do
      def size = member_ids.size
    end

    # items: Enumerable of [id, vector] pairs (vector = Array<Float>).
    def self.cluster(items, min_similarity: DEFAULT_MIN_SIMILARITY)
      new(min_similarity: min_similarity).cluster(items)
    end

    def initialize(min_similarity: DEFAULT_MIN_SIMILARITY)
      @min_similarity = min_similarity
    end

    def cluster(items)
      groups = []

      items.each do |id, vector|
        next if vector.nil? || vector.empty?

        best = nil
        best_sim = -1.0
        groups.each do |group|
          sim = cosine(vector, group[:centroid])
          if sim > best_sim
            best_sim = sim
            best = group
          end
        end

        if best && best_sim >= @min_similarity
          best[:ids] << id
          best[:sum] = add(best[:sum], vector)
          best[:centroid] = scale(best[:sum], 1.0 / best[:ids].size)
        else
          groups << { ids: [ id ], sum: vector.dup, centroid: vector.dup }
        end
      end

      groups
        .map { |g| Cluster.new(member_ids: g[:ids], centroid: g[:centroid]) }
        .sort_by { |c| -c.size }
    end

    private

    def cosine(a, b)
      dot = 0.0
      mag_a = 0.0
      mag_b = 0.0
      a.each_with_index do |x, i|
        y = b[i] || 0.0
        dot += x * y
        mag_a += x * x
        mag_b += y * y
      end
      denom = Math.sqrt(mag_a) * Math.sqrt(mag_b)
      denom.zero? ? 0.0 : dot / denom
    end

    def add(a, b)
      a.each_with_index.map { |x, i| x + (b[i] || 0.0) }
    end

    def scale(a, factor)
      a.map { |x| x * factor }
    end
  end
end
