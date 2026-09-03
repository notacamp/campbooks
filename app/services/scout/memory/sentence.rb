# frozen_string_literal: true

module Scout
  module Memory
    # A memory sentence, split into a run of spans that are either plain or bold.
    # Sources render an i18n template whose bold parts are marked with `**...**`
    # (mirroring Markdown), then hand the string here; the component walks the
    # spans to emit `<b>` runs. Keeping the bold structure as data (rather than
    # pre-rendered HTML) lets specs assert on it and lets the client-side search
    # filter read `#plain` without stripping tags.
    #
    #   Scout::Memory::Sentence.parse("File anything from **EDP** under **Utilities**.")
    #   #=> spans: [{text: "File anything from ", bold: false},
    #              {text: "EDP", bold: true},
    #              {text: " under ", bold: false},
    #              {text: "Utilities", bold: true},
    #              {text: ".", bold: false}]
    class Sentence
      MARKER = /(\*\*.+?\*\*)/

      attr_reader :spans

      def self.parse(string)
        new(build_spans(string.to_s))
      end

      def self.build_spans(string)
        string.split(MARKER).filter_map do |part|
          next if part.empty?

          if part.start_with?("**") && part.end_with?("**") && part.length > 4
            { text: part[2..-3], bold: true }
          else
            { text: part, bold: false }
          end
        end
      end
      private_class_method :build_spans

      def initialize(spans)
        @spans = spans
      end

      # The whole sentence as plain text (used by the client search filter and by
      # specs). Joining the span texts reproduces the original minus the markers.
      def plain
        spans.map { |s| s[:text] }.join
      end
      alias_method :to_s, :plain

      def ==(other)
        other.is_a?(Sentence) && other.spans == spans
      end
      alias_method :eql?, :==

      def hash
        spans.hash
      end
    end
  end
end
