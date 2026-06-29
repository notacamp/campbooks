module Tools
  # Links an email to an EXISTING task (a typed TaskEmailLink). Backs the
  # `link_task_to_email` action from Scout (which supplies the task_id it picked)
  # and the inbox/Cmd+K pickers. The relationship defaults to `related`. The task
  # must be accessible to the acting user (workspace-scoped). Returns the linked
  # Task, or nil if the task isn't found/accessible.
  class LinkTaskToEmail
    def self.call(email_message, args = {}, user: Current.user)
      new(email_message, args, user).call
    end

    def initialize(email_message, args, user)
      @email = email_message
      @args = (args || {}).with_indifferent_access
      @user = user
    end

    def call
      task = Task.accessible_to(@user).find_by(id: @args[:task_id])
      return nil unless task

      link = task.task_email_links.find_or_initialize_by(email_message: @email)
      link.relationship = relationship
      link.created_by = @user
      link.save ? task : nil
    end

    private

    def relationship
      key = @args[:relationship].to_s.downcase
      TaskEmailLink.relationships.key?(key) ? key : "related"
    end
  end
end
