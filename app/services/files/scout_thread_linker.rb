module Files
  # After a document is extracted + classified from an email, Scout posts a link to
  # it into that email's discussion thread, so the conversation carries the running
  # record of what was filed (Files Phase 3c). Opt-in per workspace
  # (`scout_thread_posts`, default off) and idempotent (`documents.posted_to_thread_at`),
  # so it posts at most once per document and only when the workspace asked for it.
  #
  # Reuses Discussions::ScoutAnnouncer (the same proactive-Scout path used for
  # extracted events/reminders): it resolves the mailbox owner, lazily creates the
  # discussion thread, posts the AI message, and broadcasts to live viewers — all
  # best-effort. Called from DocumentProcessJob, where Current.workspace is set.
  class ScoutThreadLinker
    def self.call(document) = new(document).call

    def initialize(document)
      @document = document
    end

    def call
      return unless Current.workspace&.scout_thread_posts?
      return if @document.posted_to_thread_at?

      email_message = @document.email_messages.first
      return unless email_message

      message = Discussions::ScoutAnnouncer.announce(email_message: email_message) { |_owner| body }
      return unless message

      @document.update_columns(posted_to_thread_at: Time.current)
      Events.publish("document.linked_to_thread", subject: @document, payload: { "title" => @document.display_title })
    rescue => e
      Rails.logger.error("[Files::ScoutThreadLinker] failed for document=#{@document.id}: #{e.message}")
    end

    private

    # Markdown (AI comments render Markdown with safe links). A relative in-app path —
    # the discussion is viewed inside the app, and the viewer is the mailbox owner.
    def body
      path = Rails.application.routes.url_helpers.document_path(@document)
      title = @document.display_title.to_s.gsub(/[\[\]]/, "")
      I18n.t("files.scout_thread_linker.body", title: title, path: path)
    end
  end
end
