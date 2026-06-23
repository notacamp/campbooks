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
end
