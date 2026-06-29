module Tools
  # Creates a Task from an email, with the email as its origin (Task#source), and
  # links it. Backs the `create_task_from_email` action across the inbox, Cmd+K,
  # Scout and workflows. Title/description/priority/due_at come from the caller's
  # args, falling back to the email's subject. A user-initiated task starts in
  # `todo` (not `suggested` — that state is for AI proposals awaiting Skim triage).
  # Returns the created Task, or nil on failure.
  class CreateTaskFromEmail
    def self.call(email_message, args = {}, user: Current.user)
      new(email_message, args, user).call
    end

    def initialize(email_message, args, user)
      @email = email_message
      @args = (args || {}).with_indifferent_access
      @user = user
    end

    def call
      workspace = @email.email_account.workspace
      task = workspace.tasks.new(
        title:       @args[:title].presence || default_title,
        description: @args[:description].presence,
        priority:    priority,
        due_at:      parse_time(@args[:due_at]),
        status:      :todo,
        created_by:  @user,
        source:      @email
      )
      task.save ? task : nil
    end

    private

    def default_title
      @email.subject.to_s.strip.presence&.first(120) ||
        "Follow up on email from #{@email.from_address}"
    end

    def priority
      key = @args[:priority].to_s.downcase
      Task.priorities.key?(key) ? key : "normal"
    end

    def parse_time(val)
      return nil if val.blank?

      Time.zone.parse(val.to_s)
    rescue ArgumentError
      nil
    end
  end
end
