namespace :search do
  desc "Reindex all email messages, contacts, documents, and tags for vector search"
  task reindex_all: :environment do
    puts "=== Search Reindex ==="
    puts

    # Emails
    email_count = EmailMessage.count
    puts "Enqueuing #{email_count} emails..."
    EmailMessage.find_each.with_index do |msg, i|
      msg.reindex_search!
      puts "  [#{i + 1}/#{email_count}] Email ##{msg.id}: #{msg.subject&.truncate(60)}" if (i + 1) % 50 == 0 || i == 0
    end
    puts "  Done. #{email_count} emails enqueued."
    puts

    # Contacts
    contact_count = Contact.count
    puts "Enqueuing #{contact_count} contacts..."
    Contact.find_each.with_index do |contact, i|
      contact.reindex_search!
      puts "  [#{i + 1}/#{contact_count}] Contact ##{contact.id}: #{contact.email}" if (i + 1) % 50 == 0 || i == 0
    end
    puts "  Done. #{contact_count} contacts enqueued."
    puts

    # Documents
    doc_count = Document.count
    puts "Enqueuing #{doc_count} documents..."
    Document.find_each.with_index do |doc, i|
      doc.reindex_search!
      puts "  [#{i + 1}/#{doc_count}] Document ##{doc.id}: #{doc.display_title.truncate(60)}" if (i + 1) % 50 == 0 || i == 0
    end
    puts "  Done. #{doc_count} documents enqueued."
    puts

    # Tags
    tag_count = Tag.count
    puts "Enqueuing #{tag_count} tags..."
    Tag.find_each.with_index do |tag, i|
      tag.send(:enqueue_tag_embedding)
      puts "  [#{i + 1}/#{tag_count}] Tag ##{tag.id}: #{tag.name}" if (i + 1) % 50 == 0 || i == 0
    end
    puts "  Done. #{tag_count} tags enqueued."
    puts

    puts "=== All jobs enqueued. Run bin/rails solid_queue:start to process. ==="
  end

  desc "Show search index statistics"
  task stats: :environment do
    puts "=== Search Index Stats ==="
    puts
    puts "SearchRecords: #{SearchRecord.count} total"
    SearchRecord.group(:searchable_type).count.each do |type, count|
      puts "  #{type}: #{count}"
    end
    puts
    puts "SearchChunks: #{SearchChunk.count} total"
    SearchChunk.group(:searchable_type).count.each do |type, count|
      puts "  #{type}: #{count}"
    end
    puts
    embedded = SearchChunk.where.not(embedding: nil).count
    total = SearchChunk.count
    puts "Chunks embedded: #{embedded}/#{total} (#{total > 0 ? (embedded * 100 / total) : 0}%)"
    puts
    puts "Tag embeddings: #{SearchTagEmbedding.count}"
  end

  desc "Clear all search index data"
  task clear: :environment do
    SearchRecord.delete_all
    SearchChunk.delete_all
    SearchTagEmbedding.delete_all
    puts "Search index cleared."
  end

  desc "Backfill filter_data + tags on existing EmailMessage search records (no re-embed)"
  task backfill_email_filter_data: :environment do
    scope = SearchRecord.where(searchable_type: "EmailMessage")
    total = scope.count
    puts "=== Backfilling filter_data for #{total} EmailMessage search records (no re-embed) ==="

    updated = 0
    orphaned = 0
    scope.find_each.with_index do |sr, i|
      msg = EmailMessage.find_by(id: sr.searchable_id)
      unless msg
        orphaned += 1
        next
      end

      sr.update_columns(
        filter_data: msg.searchable_filter_data,
        tags: msg.searchable_tags,
        source_updated_at: Time.current
      )
      updated += 1
      puts "  [#{i + 1}/#{total}] refreshed search_record ##{sr.id} (EmailMessage ##{sr.searchable_id})" if (i + 1) % 100 == 0 || i == 0
    end

    puts "=== Done. #{updated} refreshed, #{orphaned} orphaned records skipped. ==="
  end
end
