module Events
  # Records an Event and fans it out to any workflows listening for it.
  #
  # Contract:
  #   * Fail-safe — tracking must NEVER break the domain action that triggered
  #     it. Any error is logged/reported and swallowed; the method returns nil.
  #   * Workspace resolution: explicit `workspace:` → subject.workspace (when it
  #     responds) → Current.workspace. With none resolvable, the event is dropped
  #     (returns nil) rather than raising.
  #   * Actor: the `:current` sentinel resolves to Current.user; pass `actor: nil`
  #     for an explicit system event.
  #   * No-listener skip: the EventTriggerJob is only enqueued when the workspace
  #     actually has an enabled "event"-triggered workflow. The Event row is
  #     always written regardless (the activity feed needs it).
  class Publisher
    def self.call(name, **options)
      new(name, **options).call
    end

    def initialize(name, subject: nil, actor: :current, workspace: nil, payload: {}, occurred_at: nil, caused_by: nil)
      @name = name.to_s
      @subject = subject
      @actor = actor
      @workspace = workspace
      @payload = payload || {}
      @occurred_at = occurred_at
      @caused_by = caused_by
    end

    def call
      workspace = resolve_workspace
      return nil unless workspace

      event = Event.create!(
        workspace: workspace,
        name: @name,
        subject: @subject,
        actor: resolve_actor,
        payload: @payload,
        caused_by_event: @caused_by,
        depth: @caused_by ? @caused_by.depth + 1 : 0,
        occurred_at: @occurred_at || Time.current
      )

      track_metric

      Workflows::EventTriggerJob.perform_later(event.id) if Features.workflows? && listeners?(workspace)

      event
    rescue StandardError => e
      Rails.logger.error("[Events::Publisher] failed to publish #{@name.inspect}: #{e.class}: #{e.message}")
      Sentry.capture_exception(e) if defined?(Sentry)
      nil
    end

    private

    def resolve_workspace
      return @workspace if @workspace
      return @subject.workspace if @subject.respond_to?(:workspace) && @subject.workspace
      Current.workspace
    end

    def resolve_actor
      @actor == :current ? Current.user : @actor
    end

    # Count the published action for the /metrics endpoint. The event label is
    # bounded to registered Registry keys (any other name buckets as "custom")
    # so arbitrary Events.publish names can't blow up Prometheus cardinality.
    # Fail-safe: a metrics error must never change what Publisher#call returns.
    def track_metric
      definition = Events::Registry.definition(@name)
      Yabeda.campbooks.domain_events_total.increment(
        { event: definition ? @name : "custom", group: (definition&.group || :custom).to_s }
      )
    rescue StandardError => e
      Rails.logger.error("[Events::Publisher] metric failed: #{e.class}: #{e.message}")
    end

    # Skip the fan-out job entirely when nothing is listening, so high-volume
    # events don't flood the queue with no-op jobs.
    def listeners?(workspace)
      workspace.workflows.enabled.where(trigger_type: "event").exists?
    end
  end
end
