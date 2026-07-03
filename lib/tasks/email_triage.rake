namespace :emails do
  desc "Backfill EmailMessage#category via the rules categorizer (rung 1), " \
       "including the Gmail category-label hint (provider_labels / kind=category tags). " \
       "Read-only by default — pass WRITE=1 to persist. ALL=1 re-categorizes already-categorized mail."
  task categorize: :environment do
    write = ENV["WRITE"] == "1"
    # :tags preloaded for the legacy provider-hint fallback (mail ingested
    # before provider_labels existed reads its synced CATEGORY_* tags instead).
    scope = EmailMessage.includes(:tags)
    scope = scope.where(category: nil) unless ENV["ALL"] == "1"

    total = scope.count
    counts = Hash.new(0)
    done = 0

    scope.find_each(batch_size: 500) do |email|
      result = Emails::Categorizer.new(email).call
      counts[result.category] += 1
      if write
        email.update_columns(
          category: result.category.to_s,
          category_confidence: result.confidence,
          categorized_at: Time.current
        )
      end
      done += 1
    end

    safe = ->(n) { 100.0 * n / [ done, 1 ].max }
    cheap = %i[notifications promotions social updates].sum { |c| counts[c] }

    puts ""
    puts "#{write ? 'WROTE' : 'DRY RUN (no writes)'} — categorized #{done} of #{total} email(s)"
    puts "-" * 52
    counts.sort_by { |_, n| -n }.each do |category, n|
      puts format("  %-13s %6d   %5.1f%%", category, n, safe.call(n))
    end
    puts "-" * 52
    puts format("  cheap-path noise (skip LLM):  %6d   %5.1f%%", cheap, safe.call(cheap))
    puts format("  important -> full LLM:        %6d   %5.1f%%", counts[:important], safe.call(counts[:important]))
    puts format("  personal/unknown:             %6d   %5.1f%%  [cheap rung first, LLM only on embedding miss]",
                counts[:personal] + counts[:unknown], safe.call(counts[:personal] + counts[:unknown]))
  end
end
