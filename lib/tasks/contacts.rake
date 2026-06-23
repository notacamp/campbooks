namespace :contacts do
  desc "Enqueue analysis for all contacts that haven't been analyzed yet"
  task analyze_all: :environment do
    scope = Contact.needs_analysis

    total = scope.count
    if total == 0
      puts "All contacts have been analyzed. Nothing to do."
      exit
    end

    puts "Enqueuing analysis for #{total} contacts..."

    scope.find_each.with_index do |contact, i|
      ContactAnalysisJob.perform_later(contact.id)
      puts "[#{i + 1}/#{total}] Enqueued Contact ##{contact.id} (#{contact.email})"
    end

    puts "\nDone. #{total} jobs enqueued. They will be processed by Solid Queue."
  end

  desc "Enqueue analysis for all contacts with stale analysis (30+ days old)"
  task reanalyze_stale: :environment do
    stale = Contact.analyzed.where(analyzed_at: ...30.days.ago)

    total = stale.count
    if total == 0
      puts "No contacts with stale analysis. Nothing to do."
      exit
    end

    puts "Enqueuing reanalysis for #{total} contacts with stale analysis..."

    stale.find_each.with_index do |contact, i|
      ContactAnalysisJob.perform_later(contact.id, force: true)
      puts "[#{i + 1}/#{total}] Enqueued Contact ##{contact.id} (#{contact.email}) — last analyzed #{contact.analyzed_at.strftime('%Y-%m-%d')}"
    end

    puts "\nDone. #{total} jobs enqueued."
  end

  desc "Reindex all contacts in OpenSearch"
  task reindex: :environment do
    total = Contact.count
    puts "Reindexing #{total} contacts..."

    Contact.reindex

    puts "Done. #{total} contacts indexed."
  end
end
