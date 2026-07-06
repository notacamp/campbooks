# frozen_string_literal: true

class AddCaptureToExternalServiceCalls < ActiveRecord::Migration[8.1]
  def change
    add_column :external_service_calls, :request_headers,  :jsonb
    add_column :external_service_calls, :response_headers, :jsonb
    add_column :external_service_calls, :request_body,     :text
    add_column :external_service_calls, :response_body,    :text
  end
end
