module Integrations
  # Resolves the context-appropriate file source into a uniform list of descriptors
  # the Drive/Notion services can upload:
  #   - documents context  → the Document's original_file
  #   - email context      → the EmailMessage's attachments (all, or a selected subset)
  #
  #   files = Integrations::FileSource.for(document: doc)
  #   files = Integrations::FileSource.for(email_message: msg, blob_ids: %w[12 15])
  #   files.each { |f| f.open { |io| ... } }
  class FileSource
    # Wraps one ActiveStorage blob with a uniform interface. `open` yields a rewound
    # Tempfile (usable both as an IO for Notion and via #path for Drive); it is
    # cleaned up when the block returns.
    Descriptor = Struct.new(:blob, keyword_init: true) do
      def filename
        blob.filename.to_s
      end

      def content_type
        blob.content_type.presence || "application/octet-stream"
      end

      def byte_size
        blob.byte_size
      end

      def open(&block)
        base = File.basename(filename, ".*")
        ext = File.extname(filename)
        Tempfile.create([ base.presence || "file", ext ]) do |tf|
          tf.binmode
          blob.download { |chunk| tf.write(chunk) }
          tf.flush
          tf.rewind
          block.call(tf)
        end
      end
    end

    def self.for(document: nil, email_message: nil, blob_ids: nil)
      new(document: document, email_message: email_message, blob_ids: blob_ids).descriptors
    end

    def initialize(document: nil, email_message: nil, blob_ids: nil)
      @document = document
      @email_message = email_message
      @blob_ids = blob_ids.present? ? Array(blob_ids).map(&:to_s) : nil
    end

    def descriptors
      if @document
        return [] unless @document.original_file.attached?
        [ Descriptor.new(blob: @document.original_file.blob) ]
      elsif @email_message
        blobs = @email_message.files.blobs.to_a
        blobs = blobs.select { |b| @blob_ids.include?(b.id.to_s) } if @blob_ids
        blobs.map { |b| Descriptor.new(blob: b) }
      else
        []
      end
    end
  end
end
