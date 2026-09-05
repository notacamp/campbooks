namespace :attention do
  desc "Backfill attention weights for every user — idempotent"
  task backfill: :environment do
    scope = User.where.not(workspace_id: nil)
    total = scope.count
    puts "Backfilling attention weights for #{total} user(s)..."

    scope.find_each.with_index(1) do |user, i|
      count = Attention::Refresh.call(user)
      puts "  [#{i}/#{total}] #{user.email_address}: #{count} row(s)"
    end

    puts "Done."
  end

  desc "Refresh attention weights for one user by email"
  task :refresh, [ :email ] => :environment do |_t, args|
    email = args[:email]
    user  = User.find_by(email_address: email)

    unless user
      puts "User not found: #{email}"
      exit 1
    end

    count = Attention::Refresh.call(user)
    puts "#{user.email_address}: #{count} row(s) written"
  end

  desc "Print top attention weights for a user (default limit 15)"
  task :top, [ :email, :limit ] => :environment do |_t, args|
    email = args[:email]
    limit = (args[:limit] || 15).to_i
    user  = User.find_by(email_address: email)

    unless user
      puts "User not found: #{email}"
      exit 1
    end

    weights = Attention::Weights.new(user).top(limit)

    if weights.empty?
      puts "No attention weights found for #{email}. Run attention:refresh[#{email}] first."
      next
    end

    puts format("%-6s  %-10s  %-14s  %-40s  %s", "weight", "confidence", "type", "name", "reasons")
    puts "-" * 100

    weights.each do |aw|
      subject_name = begin
        case aw.subject_type
        when "Person"       then aw.subject&.display_name || "Unknown"
        when "Organization" then aw.subject&.name || "Unknown"
        else "Unknown"
        end
      rescue
        "Unknown"
      end

      reason_text = aw.reason_values.map(&:sentence).join("; ")
      puts format("%-6s  %-10s  %-14s  %-40s  %s",
                  aw.weight.round(4),
                  aw.confidence.round(4),
                  aw.subject_type,
                  subject_name.to_s.truncate(40),
                  reason_text)
    end
  end
end
