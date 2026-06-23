namespace :feed do
  desc "Backfill the home feed (feed_items) for every user — idempotent"
  task backfill: :environment do
    scope = User.where.not(workspace_id: nil)
    total = scope.count
    puts "Backfilling feed for #{total} user(s)…"

    scope.find_each.with_index(1) do |user, i|
      count = Feed::Generator.for_user(user)
      puts "  [#{i}/#{total}] #{user.email_address}: #{count} item(s)"
    end

    puts "Done."
  end
end
