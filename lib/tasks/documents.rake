namespace :documents do
  desc "Backfill a category on document types that have none (so they show in the reclassify picker and group into the right review ring)"
  task backfill_document_type_categories: :environment do
    scope = DocumentType.where("category IS NULL OR category = ''")

    total = scope.count
    if total.zero?
      puts "Every document type already has a category. Nothing to do."
      exit
    end

    puts "Backfilling categories for #{total} document type(s)..."

    counts = Hash.new(0)
    scope.find_each.with_index do |type, i|
      category = DocumentType.default_category_for(type.name)
      type.update!(category: category)
      counts[category] += 1
      puts "[#{i + 1}/#{total}] WS ##{type.workspace_id} #{type.name.inspect} → #{category}"
    end

    puts "\nDone. #{total} updated: #{counts.sort_by { |_, n| -n }.map { |c, n| "#{c}=#{n}" }.join(', ')}"
  end

  desc "Reprocess review-queue documents that have NO extracted field values (stale rows " \
       "analysed under an earlier AI setup). DRY_RUN=true previews; LIMIT=<n> caps the run; " \
       "WORKSPACE_ID=<id> scopes. Only touches analyzable (PDF/image/Word) pending documents. " \
       "Synchronous + idempotent — a re-run only re-touches whatever is still blank."
  task reprocess_blank: :environment do
    dry_run = ENV.fetch("DRY_RUN", "true") != "false"
    limit   = ENV["LIMIT"]&.to_i
    ws_id   = ENV["WORKSPACE_ID"]&.to_i
    banner  = dry_run ? "DRY RUN (set DRY_RUN=false to apply)" : "APPLYING CHANGES"
    puts "=== documents:reprocess_blank — #{banner}#{limit ? " LIMIT=#{limit}" : ''} ==="

    totals = Documents::BlankReprocessor.run(dry_run: dry_run, limit: limit, only_workspace_id: ws_id) do |doc, outcome|
      puts "  [#{outcome}] ws ##{doc.workspace_id} doc ##{doc.id} #{doc.document_type} — #{doc.original_file.filename.to_s.truncate(48)}"
    end

    puts "\nDone: #{totals.sort.map { |k, v| "#{k}=#{v}" }.join(', ')}"
  end
end
