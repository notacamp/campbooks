# frozen_string_literal: true

module Api
  module V1
    # Serializes a Task for the public API. List responses carry the core fields;
    # pass detail: true (show / create / update / complete) to add justification,
    # confidence, the typed email links, attached documents, and extracted_data.
    class TaskSerializer
      def initialize(task, detail: false)
        @task = task
        @detail = detail
      end

      def as_json
        data = {
          id:            @task.id,
          title:         @task.title,
          description:   @task.description,
          status:        @task.status,
          priority:      @task.priority,
          due_at:        @task.due_at&.iso8601,
          all_day:       @task.all_day,
          completed_at:  @task.completed_at&.iso8601,
          ai_suggested:  @task.ai_suggested,
          source_type:   @task.source_type,
          source_id:     @task.source_id,
          created_by_id: @task.created_by_id,
          assignee_ids:  @task.assignee_ids,
          tag_ids:       @task.tag_ids,
          created_at:    @task.created_at.iso8601,
          updated_at:    @task.updated_at.iso8601
        }

        if @detail
          data[:justification]   = @task.justification
          data[:confidence]      = @task.confidence
          data[:linked_emails]   = @task.task_email_links.map { |l| { email_message_id: l.email_message_id, relationship: l.relationship } }
          data[:document_ids]    = @task.document_ids
          data[:extracted_data]  = @task.extracted_data
        end

        data
      end
    end
  end
end
