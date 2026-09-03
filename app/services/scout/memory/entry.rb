# frozen_string_literal: true

require "base64"

module Scout
  module Memory
    # One line in Scout's memory — a behaviour, expressed as an editable sentence
    # with a visible origin. Entries are *derived* from the behaviour-bearing
    # records (rules, tags, contacts, learned decisions, …), never stored; the
    # catalog rebuilds them per request. See Scout::Memory::Catalog.
    #
    # id            stable string, e.g. "rule:<uuid>", "skim:domain:<token>" — used
    #               to address the entry for confirm/remove. Free-text parts are
    #               base64url-encoded (see .token) so ids are URL/route-safe and
    #               contain no "." that Rails would read as a format.
    # facet         :filing | :stack | :replies | :streams | :style
    # sentence      a Scout::Memory::Sentence (rich: plain + bold spans)
    # origin        :taught (by you) | :learned (from corrections) | :default
    # origin_detail short muted qualifier ("Jul 12", "3 corrections", "how you skim")
    # record        the backing AR record, or nil for derived/default entries
    # form_path     the existing settings panel URL that still edits the underlying
    #               field, or nil
    # actions       subset of [:edit, :remove, :confirm]
    # source_key    which Sources::* produced it — the catalog dispatches
    #               confirm/remove back to that source.
    class Entry
      ORIGINS = %i[taught learned default].freeze
      # Automations rides last and only surfaces when Features.workflows? is on
      # (its source yields nothing otherwise, so the facet chip stays hidden).
      FACETS = %i[filing stack replies streams style automations].freeze

      attr_reader :id, :facet, :sentence, :origin, :origin_detail, :record, :form_path, :actions, :source_key

      def initialize(id:, facet:, sentence:, origin:, source_key:, origin_detail: nil,
                     record: nil, form_path: nil, actions: [])
        @id = id
        @facet = facet.to_sym
        @sentence = sentence
        @origin = origin.to_sym
        @origin_detail = origin_detail
        @record = record
        @form_path = form_path
        @actions = Array(actions).map(&:to_sym)
        @source_key = source_key.to_sym
      end

      # URL/route-safe, stable token for a free-text key (domain, sender name,
      # category…). base64url, no padding — never contains "." or "/".
      def self.token(value)
        Base64.urlsafe_encode64(value.to_s, padding: false)
      end

      def taught? = origin == :taught
      def learned? = origin == :learned
      def default? = origin == :default

      def editable? = actions.include?(:edit)
      def removable? = actions.include?(:remove)
      def confirmable? = actions.include?(:confirm)

      # Plain text of the sentence, for the client-side search filter.
      def plain = sentence.plain

      def ==(other)
        other.is_a?(Entry) && other.id == id
      end
      alias_method :eql?, :==

      def hash = id.hash
    end
  end
end
