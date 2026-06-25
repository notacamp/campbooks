module Documents
  # Re-runs AI analysis on documents that came back with NO extracted field values.
  # The data is almost always extractable — these rows were analysed under an
  # earlier, weaker or misconfigured AI setup (text-only model / missing doc key)
  # and never reprocessed. Re-running with the current (vision) document provider
  # recovers them.
  #
  # Scope guards keep it cheap and safe:
  #   • only ANALYZABLE files (PDF / image / Word) — never .ics/.eml/.zip junk, which
  #     would just burn tokens and come back empty again
  #   • only the review queue by default (review_status: pending) — never re-touches
  #     an APPROVED document (re-analysis resets review_status to pending, which would
  #     silently un-approve it)
  #   • only workspaces with a usable document provider
  #
  # Idempotent: a second run only re-touches whatever is still blank. Runs the real
  # Documents::Processor synchronously so the caller sees outcomes immediately.
  class BlankReprocessor
    def self.run(dry_run: false, limit: nil, only_workspace_id: nil, &block)
      new(dry_run: dry_run, limit: limit, only_workspace_id: only_workspace_id).run(&block)
    end

    def initialize(dry_run: false, limit: nil, only_workspace_id: nil)
      @dry_run = dry_run
      @limit = limit
      @only_workspace_id = only_workspace_id
    end

    # Yields [document, outcome] for each candidate so the rake task can stream
    # progress. Outcomes: :recovered, :still_blank, :no_provider, :would_reprocess (dry).
    def run
      results = Hash.new(0)
      candidates.each do |doc|
        outcome = process(doc)
        results[outcome] += 1
        yield(doc, outcome) if block_given?
      end
      results
    end

    private

    # A file the document provider can actually read. Mirrors the analyzer's own
    # branches (PDF / image / .docx text extraction); everything else only ever
    # yields a filename guess, so reprocessing it is wasted spend.
    def analyzable?(doc)
      return false unless doc.original_file.attached?

      doc.pdf? || doc.image? ||
        doc.original_file.content_type.to_s.include?("wordprocessingml.document")
    end

    def blank?(doc)
      Documents::ExtractedFieldSet.new(doc).fields.none? { |f| f[:value].to_s.strip.present? }
    end

    def candidates
      scope = Document.needs_review.includes(:classification).with_attached_original_file
      scope = scope.where(workspace_id: @only_workspace_id) if @only_workspace_id
      picked = scope.lazy.select { |doc| analyzable?(doc) && blank?(doc) }
      picked = picked.first(@limit) if @limit
      picked.to_a
    end

    def process(doc)
      return :would_reprocess if @dry_run

      Current.workspace = doc.workspace
      return :no_provider unless Ai::ProviderSetup.configured?(doc.workspace, :documents)

      Documents::Processor.new(doc).call
      blank?(doc.reload) ? :still_blank : :recovered
    rescue => e
      Rails.logger.error("[Documents::BlankReprocessor] doc ##{doc.id} failed: #{e.message}")
      :error
    ensure
      Current.workspace = nil
    end
  end
end
