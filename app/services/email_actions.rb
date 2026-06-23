# Canonical registry of email/thread actions. One definition per action —
# metadata (label, target, permission, destructiveness, surfaces) plus how to
# execute it (wrapping the Tools::* services). Surfaces (Scout suggested + auto,
# bulk, Cmd+K, manual) should dispatch *execution* through EmailActions.run
# instead of carrying their own `case tool` ladders, and source their *labels /
# available tools* from the registry. See docs/action-system.md.
#
# run() returns the same shape the old Tools::Executor did, so existing callers
# and Turbo broadcasts keep working: { success:, tool:, message:, result: }.
class EmailActions
  Definition = Struct.new(
    :id, :label, :target, :perm, :destructive, :surfaces, :runner,
    keyword_init: true
  ) do
    # Human-facing label for a button/chip; `label` may be a String or a
    # proc(args) for value-dependent labels ("Tag: invoice").
    def label_for(args = {})
      label.respond_to?(:call) ? label.call((args || {}).with_indifferent_access) : label
    end

    def sends?
      perm == :send
    end
  end

  # --- public API -----------------------------------------------------------

  def self.registry
    @registry ||= DEFINITIONS.index_by(&:id).freeze
  end

  def self.definition(tool)
    registry[tool.to_s]
  end

  def self.tools
    registry.keys
  end

  # Tools exposed on a given surface (:single, :bulk, :palette, :scout_suggest,
  # :scout_auto). Lets each surface generate its commands from one place.
  def self.tools_for(surface)
    registry.values.select { |d| d.surfaces.include?(surface) }
  end

  # Server-side gate for the auto-execution path in background jobs (Scout email
  # reply, global Scout chat). Returns true only when ALL three conditions hold:
  #   1. The action is registered (unknown tool keys are never safe).
  #   2. The action is NOT destructive (destructive: true).
  #   3. The action does NOT require send permission (perm: :send).
  #   4. The action explicitly lists :scout_auto in its surfaces.
  #
  # Defense-in-depth: this predicate is evaluated server-side in Ruby, independent
  # of any prompt or model output, so a prompt-injection or jailbreak in an email
  # body cannot bypass it.
  def self.auto_safe?(action_key)
    defn = definition(action_key.to_s)
    return false unless defn
    return false if defn.destructive
    return false if defn.sends?
    defn.surfaces.include?(:scout_auto)
  end

  # Execute an action. email_message is required for message/thread-scoped
  # tools; bulk tools read their targets from args. Returns
  # { success:, tool:, message:, result: }.
  def self.run(tool, email_message: nil, args: {}, user: Current.user)
    tool = tool.to_s
    # Accept ActionController::Parameters (controller args), a JSON string (some
    # callers), or a plain Hash. Tool args are simple scalars, never mass-assigned.
    args = args.to_unsafe_h if args.respond_to?(:to_unsafe_h)
    args = JSON.parse(args) if args.is_a?(String)
    args = (args || {}).to_h.with_indifferent_access

    defn = definition(tool)
    return failure(tool, "Unknown action: #{tool}") unless defn

    # Permission gate for message/thread-scoped tools. Fails closed when no
    # acting user is established (Executor runs in background jobs).
    if email_message
      account = email_message.email_account
      return failure(tool, I18n.t("email_actions.access_denied")) unless user && account.accessible_by?(user)
      return failure(tool, I18n.t("email_actions.send_permission_denied")) if defn.sends? && !account.sendable_by?(user)
    end

    defn.runner.call(email_message, args).merge(tool: tool)
  rescue => e
    Rails.logger.error("[EmailActions] #{tool} failed: #{e.class}: #{e.message}")
    failure(tool, "#{tool} failed: #{e.message}")
  end

  def self.failure(tool, message)
    { success: false, tool: tool.to_s, message: message, result: nil }
  end

  # --- sender-scoped helpers ------------------------------------------------

  # Resolve the sender Contact for a sender-scoped action, identifying one if the
  # message was never linked (legacy mail). Returns nil only when unresolvable.
  def self.sender_contact(email_message)
    return nil unless email_message

    email_message.contact || Contacts::Identifier.contact_for(email_message)
  end

  # Run a sender-scoped action: resolve the Contact and yield it, or return a
  # normalized failure when there's no sender to act on. (run() merges :tool.)
  def self.with_sender(email_message)
    contact = sender_contact(email_message)
    return { success: false, message: I18n.t("email_actions.sender_not_found"), result: nil } unless contact

    yield contact
  end

  # --- the registry ---------------------------------------------------------

  DEFINITIONS = [
    Definition.new(
      id: "add_tag", target: :message, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest scout_auto workflow],
      label: ->(a) { "Tag: #{a[:tag_name]}" },
      runner: ->(msg, args) {
        tag = Tools::AddTag.call(msg, args)
        tag ? { success: true, message: I18n.t("email_actions.add_tag.success", tag_name: tag.name), result: { tag: { id: tag.id, name: tag.name, color: tag.color } } }
            : { success: false, message: I18n.t("email_actions.add_tag.not_found", tag_name: args[:tag_name]), result: nil }
      }
    ),
    Definition.new(
      id: "remove_tag", target: :message, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest scout_auto workflow],
      label: ->(a) { "Remove tag: #{a[:tag_name]}" },
      runner: ->(msg, args) {
        tag = Tools::RemoveTag.call(msg, args)
        tag ? { success: true, message: I18n.t("email_actions.remove_tag.success", tag_name: tag.name), result: { tag: { id: tag.id, name: tag.name } } }
            : { success: false, message: I18n.t("email_actions.remove_tag.not_found", tag_name: args[:tag_name]), result: nil }
      }
    ),
    Definition.new(
      id: "archive", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest scout_auto skim board workflow],
      label: "Archive",
      runner: ->(msg, args) {
        Tools::Archive.call(msg, args) ? { success: true, message: I18n.t("email_actions.archive.success"), result: { archived: true } }
                                       : { success: false, message: I18n.t("email_actions.archive.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "trash", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest scout_auto workflow],
      label: "Move to Trash",
      runner: ->(msg, args) {
        Tools::Trash.call(msg, args) ? { success: true, message: I18n.t("email_actions.trash.success"), result: { trashed: true } }
                                     : { success: false, message: I18n.t("email_actions.trash.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "snooze", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest scout_auto board],
      label: "Snooze",
      runner: ->(msg, args) {
        thread = Tools::Snooze.call(msg, args)
        thread ? { success: true, message: I18n.t("email_actions.snooze.success", snoozed_until: thread.snoozed_until), result: { snoozed: true, snoozed_until: thread.snoozed_until } }
               : { success: false, message: I18n.t("email_actions.snooze.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "unsnooze", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single palette scout_suggest scout_auto board],
      label: "Unsnooze",
      runner: ->(msg, _args) {
        Tools::Unsnooze.call(msg) ? { success: true, message: I18n.t("email_actions.unsnooze.success"), result: { unsnoozed: true } }
                                  : { success: false, message: I18n.t("email_actions.unsnooze.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "unarchive", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single palette skim board],
      label: "Move to Inbox",
      runner: ->(msg, _args) {
        Tools::Unarchive.call(msg) ? { success: true, message: I18n.t("email_actions.unarchive.success"), result: { unarchived: true } }
                                   : { success: false, message: I18n.t("email_actions.unarchive.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "forward_email", target: :thread, perm: :send, destructive: false,
      surfaces: %i[single scout_suggest scout_auto workflow],
      label: ->(a) { "Forward to #{a[:to_address]}" },
      runner: ->(msg, args) {
        Tools::ForwardEmail.call(msg, args) ? { success: true, message: I18n.t("email_actions.forward_email.success", to_address: args[:to_address]), result: { forwarded: true, to_address: args[:to_address] } }
                                            : { success: false, message: I18n.t("email_actions.forward_email.failure", to_address: args[:to_address]), result: nil }
      }
    ),
    Definition.new(
      id: "create_calendar_event", target: :message, perm: :read, destructive: false,
      surfaces: %i[single palette scout_suggest workflow],
      label: ->(a) { a[:title].present? ? "Create event: #{a[:title]}" : "Create calendar event" },
      runner: ->(msg, args) {
        event = Tools::CreateCalendarEvent.call(msg, args)
        event ? { success: true, message: I18n.t("email_actions.create_calendar_event.success", title: event.title), result: { event_id: event.id, title: event.title } }
              : { success: false, message: I18n.t("email_actions.create_calendar_event.no_calendar"), result: nil }
      }
    ),
    Definition.new(
      id: "upload_attachments_to_drive", target: :message, perm: :read, destructive: false,
      surfaces: %i[palette scout_suggest],
      label: ->(a) { a[:folder_name].present? ? "Upload attachments to Drive: #{a[:folder_name]}" : "Upload attachments to Drive" },
      runner: ->(msg, args) {
        res = Tools::UploadEmailAttachmentsToDrive.call(msg, args)
        res ? { success: true, message: I18n.t("email_actions.upload_attachments_to_drive.success", count: res[:count]), result: res }
            : { success: false, message: I18n.t("email_actions.upload_attachments_to_drive.failure"), result: nil }
      }
    ),
    Definition.new(
      id: "reclassify", target: :bulk, perm: :read, destructive: false,
      surfaces: %i[scout_auto],
      label: "Re-classify",
      runner: ->(_msg, args) {
        r = Tools::Reclassify.call(args)
        { success: true, message: I18n.t("email_actions.reclassify.success", count: r[:reclassified_count]), result: r }
      }
    ),
    Definition.new(
      id: "bulk_archive", target: :bulk, perm: :read, destructive: false,
      surfaces: %i[scout_auto],
      label: "Archive selected",
      runner: ->(_msg, args) {
        r = Tools::BulkArchive.call(args)
        { success: true, message: I18n.t("email_actions.bulk_archive.success", count: r[:archived_count]), result: r }
      }
    ),
    Definition.new(
      id: "bulk_tag", target: :bulk, perm: :read, destructive: false,
      surfaces: %i[scout_auto],
      label: ->(a) { "#{a[:action] == 'remove' ? 'Remove' : 'Add'} tag: #{a[:tag_name]}" },
      runner: ->(_msg, args) {
        r = Tools::BulkTag.call(args)
        if r[:error]
          { success: false, message: r[:error], result: r }
        else
          key = r[:action] == "remove" ? "email_actions.bulk_tag.removed" : "email_actions.bulk_tag.added"
          { success: true, message: I18n.t(key, tag_name: r[:tag_name], count: r[:tagged_count]), result: r }
        end
      }
    ),

    # --- sender-scoped actions ------------------------------------------------
    # Mutate the message's Contact (the normalized, workspace-scoped sender), not
    # the message. The runner resolves the Contact from the email_message and the
    # standard accessible_by? gate in run() still applies (all are perm: :read).
    Definition.new(
      id: "star_sender", target: :sender, perm: :read, destructive: false,
      surfaces: %i[single bulk palette scout_suggest skim],
      label: "Star sender",
      runner: ->(msg, _args) {
        EmailActions.with_sender(msg) do |c|
          c.star!
          { success: true, message: I18n.t("email_actions.star_sender.success", name: c.display_name), result: { contact_id: c.id, starred: true } }
        end
      }
    ),
    Definition.new(
      id: "unstar_sender", target: :sender, perm: :read, destructive: false,
      surfaces: %i[single palette skim],
      label: "Unstar sender",
      runner: ->(msg, _args) {
        EmailActions.with_sender(msg) do |c|
          c.unstar!
          { success: true, message: I18n.t("email_actions.unstar_sender.success", name: c.display_name), result: { contact_id: c.id, starred: false } }
        end
      }
    ),
    Definition.new(
      id: "block_sender", target: :sender, perm: :read, destructive: true,
      surfaces: %i[single bulk palette scout_suggest skim],
      label: "Block sender",
      runner: ->(msg, _args) {
        EmailActions.with_sender(msg) do |c|
          c.block!
          SenderBlockArchiveJob.perform_later(c.id, Current.user&.id)
          { success: true, message: I18n.t("email_actions.block_sender.success", name: c.display_name), result: { contact_id: c.id, list_status: c.list_status } }
        end
      }
    ),
    Definition.new(
      id: "unblock_sender", target: :sender, perm: :read, destructive: false,
      surfaces: %i[single palette skim],
      label: "Unblock sender",
      runner: ->(msg, _args) {
        EmailActions.with_sender(msg) do |c|
          c.unblock!
          { success: true, message: I18n.t("email_actions.unblock_sender.success", name: c.display_name), result: { contact_id: c.id, list_status: c.list_status } }
        end
      }
    ),
    Definition.new(
      id: "allow_sender", target: :sender, perm: :read, destructive: false,
      surfaces: %i[single palette skim],
      label: "Allow sender",
      runner: ->(msg, _args) {
        EmailActions.with_sender(msg) do |c|
          c.allow!
          { success: true, message: I18n.t("email_actions.allow_sender.success", name: c.display_name), result: { contact_id: c.id, list_status: c.list_status } }
        end
      }
    ),

    # --- priority (pin) -------------------------------------------------------
    # Pin/unpin a thread into the inbox "Priority" section. Same pinned_at field
    # Skim's "Make priority" writes, so the two stay in sync. Pinning the row's
    # message is enough to mark the thread pinned; unpin clears the whole thread.
    Definition.new(
      id: "pin", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single palette],
      label: "Make priority",
      runner: ->(msg, _args) {
        msg.update!(pinned_at: Time.current)
        { success: true, message: I18n.t("email_actions.pin.success"), result: { pinned: true, thread_id: msg.email_thread_id } }
      }
    ),
    Definition.new(
      id: "unpin", target: :thread, perm: :read, destructive: false,
      surfaces: %i[single palette],
      label: "Remove from priority",
      runner: ->(msg, _args) {
        if (thread = msg.email_thread)
          thread.email_messages.update_all(pinned_at: nil, updated_at: Time.current)
        else
          msg.update!(pinned_at: nil)
        end
        { success: true, message: I18n.t("email_actions.unpin.success"), result: { pinned: false, thread_id: msg.email_thread_id } }
      }
    )
  ].freeze
end
