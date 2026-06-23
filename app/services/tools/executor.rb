module Tools
  # Thin shim kept for the background jobs (Scout email reply, the reply daemon,
  # global Scout chat) that call Tools::Executor.call(...). All execution and
  # permission logic now lives in the EmailActions registry — this only adapts
  # the keyword signature and supplies the acting user.
  # See app/services/email_actions.rb and docs/action-system.md.
  class Executor
    def self.call(tool:, email_message: nil, args: {})
      EmailActions.run(tool, email_message: email_message, args: args, user: Current.user)
    end
  end
end
