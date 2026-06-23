module Exports
  class ZipGenerator
    def initialize(documents)
      @documents = documents
    end

    def call
      buffer = Zip::OutputStream.write_buffer do |zip|
        @documents.each do |document|
          folder = document.classification&.name || "Other"
          blob = document.processed_pdf.attached? ? document.processed_pdf.blob : document.original_file.blob
          filename = document.canonical_filename.presence || blob.filename.to_s

          zip.put_next_entry("#{folder}/#{filename}")
          zip.write(blob.download)
        end
      end

      buffer.string
    end
  end
end
