namespace :ai do
  desc "Re-point managed (Campbooks AI) text adapters to the current EU default (Mistral) and normalize stale models. Idempotent."
  task repoint_managed_text: :environment do
    target = Ai::Platform::MANAGED_TEXT_PROVIDER
    moved = Ai::ManagedTextRepointer.run

    if moved.empty?
      puts "Nothing to do — all managed text adapters are on #{target} with valid models."
    else
      moved.each do |m|
        change = m[:from] == m[:to] ? "model fix" : "#{m[:from]} -> #{m[:to]}"
        puts "  workspace #{m[:workspace_id]}: managed text #{change} (#{m[:models_fixed]} model(s) normalized)"
      end
      puts "Updated #{moved.size} managed text adapter(s) for #{target}."
    end
  end

  desc "Backfill tags for already-ingested emails that have none. ~2 LLM calls per email. " \
       "ENV: LIMIT=<n> (cap, recommended), WORKSPACE_ID=<id>, DAYS=<n> (only mail newer than n days). " \
       "Skips workspaces with no AI text provider or no tags the model can assign."
  task retag_untagged: :environment do
    limit        = ENV["LIMIT"]&.to_i
    workspace_id = ENV["WORKSPACE_ID"]&.to_i
    days         = ENV["DAYS"]&.to_i

    workspaces = Workspace.all
    workspaces = workspaces.where(id: workspace_id) if workspace_id

    # A workspace is only worth backfilling if it has a usable text provider (the
    # strict EmailProcessJob gate) AND at least one tag with a prompt for the model
    # to assign — otherwise every email just burns a pre-screen call and tags nothing.
    eligible = workspaces.select do |ws|
      Ai::ProviderSetup.configured?(ws, :text) &&
        ws.tags.joins(:rich_text_prompt).where.not(action_text_rich_texts: { body: [ nil, "" ] }).exists?
    end

    if eligible.empty?
      puts "No eligible workspaces (AI-configured + has taggable tags) — nothing to enqueue."
      next
    end

    account_ids = EmailAccount.where(workspace_id: eligible.map(&:id)).pluck(:id)
    scope = EmailMessage
              .where(email_account_id: account_ids)
              .where(status: EmailMessage.statuses[:processed])
              .where.missing(:email_message_tags)
              .order(received_at: :desc)
    scope = scope.where("received_at > ?", days.days.ago) if days&.positive?

    total = scope.count
    target_count = limit ? [ limit, total ].min : total
    puts "Eligible workspaces: #{eligible.map { |w| "##{w.id} #{w.name}" }.join(', ')}"
    puts "Untagged processed emails matching: #{total}#{days ? " (last #{days}d)" : ''}"
    puts "Enqueuing EmailRetagJob for #{target_count}#{limit ? " (LIMIT=#{limit})" : ' (no LIMIT — all of them)'}..."

    enqueued = 0
    scope.limit(limit).find_each do |email|
      EmailRetagJob.perform_later(email.id)
      enqueued += 1
    end
    puts "Enqueued #{enqueued} EmailRetagJob(s)."
  end
end
