namespace :email do
  desc "Re-classify processed emails that have no tags"
  task retry_classification: :environment do
    scope = EmailMessage.processed.where.missing(:email_message_tags)

    total = scope.count
    if total == 0
      puts "All processed emails have tags. Nothing to do."
      exit
    end

    puts "Found #{total} processed emails without tags. Classifying..."
    success = 0
    failed = 0

    scope.find_each.with_index do |email, i|
      print "[#{i + 1}/#{total}] Email ##{email.id} (#{email.subject.to_s.truncate(50)}) ... "
      begin
        Ai::EmailClassifier.new(email).classify!
        if email.tags.any?
          puts "OK (#{email.tags.pluck(:name).join(', ')})"
          success += 1
        else
          puts "NO TAGS ASSIGNED"
          failed += 1
        end
      rescue => e
        puts "ERROR: #{e.message}"
        failed += 1
      end
    end

    puts "\nDone. #{success} classified, #{failed} failed, #{total} total."
  end
end
