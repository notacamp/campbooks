namespace :emails do
  desc "Merge EmailThreads split across the same conversation (DRY_RUN=true by default). " \
       "Groups by (email_account_id, subject_key); keeps one canonical thread per group, " \
       "repoints its messages, merges follow-up/snooze/reply state, deletes the emptied rows. " \
       "Run DRY_RUN=false to actually write. See AddThreadKeysToEmails + Emails::SubjectNormalizer."
  task merge_split_threads: :environment do
    dry_run  = ENV.fetch("DRY_RUN", "true") != "false"
    max_span = ENV.fetch("MAX_SPAN_DAYS", "90").to_i
    banner   = dry_run ? "DRY RUN (no writes — set DRY_RUN=false to apply)" : "APPLYING CHANGES"
    puts "=== emails:merge_split_threads — #{banner} (MAX_SPAN_DAYS=#{max_span}) ==="

    groups = EmailThread.where.not(subject_key: "").where.not(subject_key: nil)
                        .group(:email_account_id, :subject_key)
                        .having("COUNT(*) > 1")
                        .count.keys

    merged = removed = repointed = skipped = 0

    groups.each do |account_id, subject_key|
      threads = EmailThread.where(email_account_id: account_id, subject_key: subject_key)
                           .includes(:agent_thread, :email_messages).to_a
      next if threads.size < 2

      with_discussion = threads.select { |t| t.agent_thread.present? }
      if with_discussion.size > 1
        skipped += 1
        puts "  SKIP (#{with_discussion.size} discussions, manual review): " \
             "acct=#{account_id} key=#{subject_key.truncate(50).inspect} threads=#{threads.map(&:id)}"
        next
      end

      # A real prefix/forward split clusters in time. A generic subject reused
      # across months/years (recurring invoice, annual policy notice) is NOT one
      # conversation — refuse to merge it. Provider thread ids (captured going
      # forward) disambiguate these precisely; historically we only have time.
      times = threads.flat_map { |t| t.email_messages.map(&:received_at) }.compact
      span  = times.any? ? (times.max - times.min) / 86_400.0 : 0
      if span > max_span
        skipped += 1
        puts "  SKIP (subject reused over #{span.round}d > #{max_span}d, likely distinct): " \
             "acct=#{account_id} key=#{subject_key.truncate(46).inspect} threads=#{threads.map(&:id)}"
        next
      end

      canonical = with_discussion.first || threads.min_by { |t| [ t.created_at, t.id ] }
      losers    = threads - [ canonical ]
      loser_ids = losers.map(&:id)
      msg_count = EmailMessage.where(email_thread_id: loser_ids).count

      merged    += 1
      removed   += losers.size
      repointed += msg_count

      if dry_run
        puts "  MERGE acct=#{account_id} key=#{subject_key.truncate(46).inspect}"
        puts "        keep ##{canonical.id} (#{canonical.subject.to_s.truncate(40).inspect}#{' +discussion' if canonical.agent_thread})" \
             " <- absorb #{loser_ids} (#{msg_count} messages)"
        next
      end

      EmailThread.transaction do
        # 1. Repoint every loser's messages onto the canonical thread.
        EmailMessage.where(email_thread_id: loser_ids).update_all(email_thread_id: canonical.id)

        # 2. Merge denormalized watermarks/state onto the canonical (max wins).
        all = threads
        updates = {
          last_inbound_at:  all.map(&:last_inbound_at).compact.max,
          last_outbound_at: all.map(&:last_outbound_at).compact.max,
          snoozed_until:    all.map(&:snoozed_until).compact.max
        }
        # Carry an active follow-up if the canonical lacks one.
        unless canonical.follow_up_expected?
          fu = losers.find { |t| t.follow_up_expected? && t.follow_up_dismissed_at.nil? }
          if fu
            updates.merge!(
              follow_up_expected: true,
              follow_up_at: fu.follow_up_at,
              follow_up_reason: fu.follow_up_reason,
              follow_up_outbound_message_id: fu.follow_up_outbound_message_id,
              follow_up_last_analyzed_at: fu.follow_up_last_analyzed_at
            )
          end
        end
        canonical.update_columns(updates.compact)

        # 3. Losers now own no messages and no discussion — safe to delete.
        EmailThread.where(id: loser_ids).delete_all
      end
    end

    puts "--- #{dry_run ? 'WOULD merge' : 'MERGED'}: #{merged} groups, " \
         "#{removed} duplicate threads #{dry_run ? 'removed' : 'deleted'}, " \
         "#{repointed} messages repointed, #{skipped} groups skipped ---"
    puts "Re-run with DRY_RUN=false to apply." if dry_run && merged.positive?
  end

  desc "Repair Gmail messages ingested while the attachment flag was broken (the flag never set, " \
       "so attachments were never downloaded or turned into Documents). Re-reads each candidate's " \
       "metadata from the Gmail API, flips has_attachment where the provider shows attachments, and " \
       "ingests files/Documents (DocumentProcessJob runs per new document — AI analysis cost applies). " \
       "DRY_RUN=true by default. ENV: DRY_RUN=false to apply, WINDOW_DAYS=90, ACCOUNT=<address> to limit."
  task backfill_gmail_attachments: :environment do
    dry_run = ENV.fetch("DRY_RUN", "true") != "false"
    window  = ENV.fetch("WINDOW_DAYS", "90").to_i
    banner  = dry_run ? "DRY RUN (no writes — set DRY_RUN=false to apply)" : "APPLYING CHANGES"
    puts "=== emails:backfill_gmail_attachments — #{banner} (WINDOW_DAYS=#{window}) ==="

    accounts = EmailAccount.where(provider: :google)
    accounts = accounts.where(email_address: ENV["ACCOUNT"]) if ENV["ACCOUNT"].present?

    total_flipped = total_docs = 0

    accounts.find_each do |account|
      client = account.mail_client
      candidates = account.email_messages
                          .where(has_attachment: [ false, nil ])
                          .where.not(provider_message_id: [ nil, "" ])
                          .where("received_at >= ?", window.days.ago)
      puts "#{account.email_address}: #{candidates.count} candidate message(s)"

      flipped = docs = 0
      candidates.in_batches(of: 50) do |batch|
        by_provider_id = batch.index_by(&:provider_message_id)
        begin
          client.fetch_messages(by_provider_id.keys).each do |normalized|
            next unless normalized["hasAttachment"].to_s == "1"

            email = by_provider_id[normalized["messageId"]]
            next unless email

            flipped += 1
            if dry_run
              puts "  would flip: #{email.received_at&.to_date} #{email.subject.to_s.truncate(60).inspect}"
            else
              email.update!(has_attachment: true)
              docs += Emails::AttachmentIngester.call(email, client).to_i
            end
          rescue => e
            puts "  ERROR on #{email&.id || normalized['messageId']}: #{e.class}: #{e.message.to_s.truncate(120)}"
          end
        rescue => e
          puts "  ERROR fetching batch: #{e.class}: #{e.message.to_s.truncate(160)} — skipping rest of account"
          break
        end
        sleep 0.2
      end
      puts "  -> #{flipped} message(s) #{dry_run ? 'would be' : ''} flipped, #{docs} document(s) created"
      total_flipped += flipped
      total_docs += docs
    end

    puts "--- TOTAL: #{total_flipped} message(s) #{dry_run ? 'would be' : ''} flipped, #{total_docs} document(s) created ---"
    puts "Re-run with DRY_RUN=false to apply." if dry_run && total_flipped.positive?
  end
end
