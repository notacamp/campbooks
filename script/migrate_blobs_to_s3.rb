# frozen_string_literal: true

#
# One-off: copy ActiveStorage files from the local DiskService into the S3
# bucket, so existing attachments survive the cutover to S3 object storage.
#
# WHY pre-cutover, in the OLD container: prod runs from a built image whose
# container is recreated on every deploy, and DiskService writes into the
# container's ephemeral layer. The deploy that switches storage to S3 therefore
# wipes /rails/storage — so the files must be pushed to S3 *before* that deploy,
# from the currently-running container while they still exist on disk.
#
# It talks to S3 through the aws-sdk-s3 gem directly (not config/storage.yml), so
# it runs unchanged inside the old image. Idempotent: objects already in the
# bucket are skipped, so it is safe to re-run.
#
# Run (pipe into the running container via stdin so the old image needn't ship
# this file):
#
#   ssh deploy@YOUR_HOST 'cd /path/to/compose && \
#     docker compose -f compose.prod.yml --env-file .env exec -T \
#       -e S3_ACCESS_KEY_ID=AKID -e S3_SECRET_ACCESS_KEY=SECRET \
#       -e S3_BUCKET=campbooks-production -e S3_REGION=nbg1 \
#       -e S3_ENDPOINT=https://nbg1.your-objectstorage.com \
#       campbooks-web bin/rails runner -' < script/migrate_blobs_to_s3.rb

require "aws-sdk-s3"

bucket = ENV.fetch("S3_BUCKET", "campbooks-production")
client_opts = {
  access_key_id:     ENV.fetch("S3_ACCESS_KEY_ID"),
  secret_access_key: ENV.fetch("S3_SECRET_ACCESS_KEY"),
  region:            ENV.fetch("S3_REGION", "eu-central-1")
}
# S3-compatible providers (e.g. Hetzner) need a custom endpoint + path style;
# plain AWS S3 uses neither.
if ENV["S3_ENDPOINT"].present?
  client_opts[:endpoint]         = ENV["S3_ENDPOINT"]
  client_opts[:force_path_style] = ENV.fetch("S3_FORCE_PATH_STYLE", "true") == "true"
end
client = Aws::S3::Client.new(**client_opts)

service = ActiveStorage::Blob.service
unless service.respond_to?(:path_for)
  abort "Current ActiveStorage service is #{service.class} (expected DiskService). " \
        "Run this in the pre-cutover container, before switching storage to S3."
end

uploaded = in_bucket = missing = errors = 0

ActiveStorage::Blob.find_each do |blob|
  path = service.path_for(blob.key)

  unless File.exist?(path)
    missing += 1
    next
  end

  begin
    client.head_object(bucket: bucket, key: blob.key)
    in_bucket += 1
    next
  rescue Aws::S3::Errors::ServiceError
    # Not in the bucket (or a transient error) → (re)upload. put_object
    # overwrites, so a retry is harmless.
  end

  begin
    File.open(path, "rb") do |io|
      client.put_object(bucket: bucket, key: blob.key, body: io, content_type: blob.content_type)
    end
    uploaded += 1
  rescue => e
    errors += 1
    warn "ERROR blob ##{blob.id} key=#{blob.key}: #{e.class}: #{e.message}"
  end
end

puts "Storage migration complete."
puts "  uploaded          = #{uploaded}"
puts "  already_in_bucket = #{in_bucket}"
puts "  missing_on_disk   = #{missing}   (unrecoverable — files lost to earlier deploys)"
puts "  errors            = #{errors}"
puts "  total_blobs       = #{ActiveStorage::Blob.count}"
