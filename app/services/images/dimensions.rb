module Images
  # Reads pixel dimensions straight from an image file's header bytes — no
  # ImageMagick subprocess, no full decode. Used at email ingestion to spot
  # tracking pixels / spacer images (1x1 and other degenerate sizes) before they
  # become Documents.
  #
  # Returns [width, height] in pixels, or nil when the format is unrecognised or
  # the header is truncated. Supports the formats that actually show up as inline
  # email images: PNG, GIF, JPEG, BMP, WEBP.
  module Dimensions
    module_function

    def read(data)
      return nil if data.nil?

      bytes = data.b # binary encoding for byte-accurate slicing
      return nil if bytes.bytesize < 16

      if    png?(bytes)  then png_dimensions(bytes)
      elsif gif?(bytes)  then gif_dimensions(bytes)
      elsif jpeg?(bytes) then jpeg_dimensions(bytes)
      elsif bmp?(bytes)  then bmp_dimensions(bytes)
      elsif webp?(bytes) then webp_dimensions(bytes)
      end
    end

    # --- format detection ---

    def png?(b)  = b[0, 8] == "\x89PNG\r\n\x1A\n".b
    def gif?(b)  = b[0, 6] == "GIF87a".b || b[0, 6] == "GIF89a".b
    def jpeg?(b) = b[0, 2] == "\xFF\xD8".b
    def bmp?(b)  = b[0, 2] == "BM".b
    def webp?(b) = b[0, 4] == "RIFF".b && b[8, 4] == "WEBP".b

    # --- per-format readers ---

    def png_dimensions(b)
      return nil if b.bytesize < 24 || b[12, 4] != "IHDR".b
      b[16, 8].unpack("N2") # width, height (big-endian)
    end

    def gif_dimensions(b)
      b[6, 4].unpack("v2") # width, height (little-endian, in the screen descriptor)
    end

    def bmp_dimensions(b)
      return nil if b.bytesize < 26
      w = b[18, 4].unpack1("l<")
      h = b[22, 4].unpack1("l<") # may be negative for top-down bitmaps
      [ w, h.abs ]
    end

    def webp_dimensions(b)
      return nil if b.bytesize < 30

      case b[12, 4]
      when "VP8 ".b # lossy: 14-bit dims after the 0x9D012A start code
        [ b[26, 2].unpack1("v") & 0x3FFF, b[28, 2].unpack1("v") & 0x3FFF ]
      when "VP8L".b # lossless: 14-bit dims packed after the 0x2F signature
        bits = b[21, 4].unpack1("V")
        [ (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1 ]
      when "VP8X".b # extended: 24-bit canvas dims (stored as size - 1)
        [ (b[24, 3] + "\x00".b).unpack1("V") + 1, (b[27, 3] + "\x00".b).unpack1("V") + 1 ]
      end
    end

    def jpeg_dimensions(b)
      len = b.bytesize
      i = 2
      while i < len
        i += 1 until i >= len || b.getbyte(i) == 0xFF # advance to a marker
        i += 1 while i < len && b.getbyte(i) == 0xFF   # consume the 0xFF fill byte(s)
        break if i >= len

        marker = b.getbyte(i)
        i += 1
        next  if marker >= 0xD0 && marker <= 0xD9 # standalone markers (RSTn/SOI/EOI), no length
        break if marker == 0xDA                   # SOS: image data follows, stop scanning
        break if i + 2 > len

        seg_len = b[i, 2].unpack1("n")
        break if seg_len.nil? || seg_len < 2

        # SOF0..SOF15 carry the frame size, excluding DHT(C4)/JPG(C8)/DAC(CC)
        if marker >= 0xC0 && marker <= 0xCF && ![ 0xC4, 0xC8, 0xCC ].include?(marker)
          return nil if i + 7 > len
          height = b[i + 3, 2].unpack1("n")
          width  = b[i + 5, 2].unpack1("n")
          return [ width, height ]
        end

        i += seg_len
      end
      nil
    end
  end
end
