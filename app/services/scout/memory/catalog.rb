# frozen_string_literal: true

module Scout
  module Memory
    # The whole of Scout's memory for one workspace + user: every behaviour,
    # derived on demand from its backing records and expressed as an Entry.
    # Nothing here is stored — the catalog is rebuilt each request from the
    # registered Sources.
    #
    #   catalog = Scout::Memory::Catalog.for(workspace, user)
    #   catalog.entries            # => [Entry, ...]
    #   catalog.facet_counts       # => [[:filing, 14], [:stack, 6], ...]
    #   catalog.perform(:remove, "rule:<uuid>")
    class Catalog
      # Declaration order is display order.
      SOURCES = [
        Sources::EmailRules,
        Sources::Tags,
        Sources::DocumentHabits,
        Sources::DocumentTypes,
        Sources::GroupRules,
        Sources::Filtering,
        Sources::SkimHabits,
        Sources::Replies,
        Sources::Prompts,
        Sources::Workflows,
        Sources::Defaults
      ].freeze

      FACETS = Entry::FACETS

      def self.for(workspace, user)
        new(workspace: workspace, user: user)
      end

      attr_reader :workspace, :user

      def initialize(workspace:, user:)
        @workspace = workspace
        @user = user
        @sources = SOURCES.map { |klass| klass.new(workspace: workspace, user: user) }
      end

      def entries
        @entries ||= @sources.flat_map(&:entries)
      end

      # Entries in a facet; nil / :all returns everything.
      def entries_for(facet)
        return entries if facet.blank? || facet.to_sym == :all

        entries.select { |entry| entry.facet == facet.to_sym }
      end

      # [[facet, count], ...] in canonical order, only facets with entries — feeds
      # the segmented chips.
      def facet_counts
        FACETS.filter_map do |facet|
          count = entries.count { |entry| entry.facet == facet }
          [ facet, count ] if count.positive?
        end
      end

      def total
        entries.size
      end

      def entry(id)
        entries.find { |entry| entry.id == id }
      end

      # Dispatch :confirm / :remove to the source that owns the entry. Returns a
      # (freshly re-derived) Entry to re-render on confirm, the removed Entry on a
      # successful remove, or nil when the entry is unknown or the action failed /
      # is unsupported.
      def perform(action, id)
        target = entry(id)
        return nil unless target

        source = @sources.find { |candidate| candidate.source_key == target.source_key }
        return nil unless source && target.actions.include?(action)
        return nil unless source.public_send(action, target)

        @entries = nil # the action mutated a record — rebuild so a re-find is fresh
        action == :confirm ? (entry(id) || target) : target
      end
    end
  end
end
